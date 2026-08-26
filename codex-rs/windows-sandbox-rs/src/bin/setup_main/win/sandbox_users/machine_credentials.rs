use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;
use std::ffi::OsStr;
use std::fmt;
use std::mem::size_of;
use std::ptr;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::STATUS_OBJECT_NAME_NOT_FOUND;
use windows_sys::Win32::Foundation::WAIT_ABANDONED;
use windows_sys::Win32::Foundation::WAIT_FAILED;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Security::Authentication::Identity::LSA_HANDLE;
use windows_sys::Win32::Security::Authentication::Identity::LSA_OBJECT_ATTRIBUTES;
use windows_sys::Win32::Security::Authentication::Identity::LSA_UNICODE_STRING;
use windows_sys::Win32::Security::Authentication::Identity::LsaClose;
use windows_sys::Win32::Security::Authentication::Identity::LsaFreeMemory;
use windows_sys::Win32::Security::Authentication::Identity::LsaNtStatusToWinError;
use windows_sys::Win32::Security::Authentication::Identity::LsaOpenPolicy;
use windows_sys::Win32::Security::Authentication::Identity::LsaRetrievePrivateData;
use windows_sys::Win32::Security::Authentication::Identity::LsaStorePrivateData;
use windows_sys::Win32::Security::Authentication::Identity::POLICY_CREATE_SECRET;
use windows_sys::Win32::Security::Authentication::Identity::POLICY_GET_PRIVATE_INFORMATION;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::Authorization::SDDL_REVISION_1;
use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::ReleaseMutex;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

use codex_windows_sandbox::to_wide;

const MACHINE_CREDENTIALS_VERSION: u32 = 1;
const MACHINE_CREDENTIALS_SECRET_NAME: &str = "L$OpenAI.Codex.WindowsSandbox.Users.v1";
const PROVISIONING_MUTEX_NAME: &str = "Global\\OpenAI.Codex.WindowsSandbox.Users";
const PROVISIONING_MUTEX_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)";

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct SandboxAccountCredentials {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for SandboxAccountCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SandboxAccountCredentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
pub(super) struct MachineSandboxCredentials {
    version: u32,
    pub offline: SandboxAccountCredentials,
    pub online: SandboxAccountCredentials,
}

impl MachineSandboxCredentials {
    pub(super) fn new(
        offline: SandboxAccountCredentials,
        online: SandboxAccountCredentials,
    ) -> Self {
        Self {
            version: MACHINE_CREDENTIALS_VERSION,
            offline,
            online,
        }
    }

    fn validate(&self, offline_username: &str, online_username: &str) -> Result<()> {
        ensure!(
            self.version == MACHINE_CREDENTIALS_VERSION,
            "machine sandbox credentials have unsupported version {}",
            self.version
        );
        ensure!(
            self.offline.username == offline_username && self.online.username == online_username,
            "machine sandbox credential identities do not match the requested accounts"
        );
        for account in [&self.offline, &self.online] {
            ensure!(
                !account.username.is_empty()
                    && !account.username.contains('\0')
                    && !account.password.is_empty()
                    && !account.password.contains('\0'),
                "machine sandbox credentials contain an invalid account record"
            );
        }
        Ok(())
    }
}

impl fmt::Debug for MachineSandboxCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MachineSandboxCredentials")
            .field("version", &self.version)
            .field("offline", &self.offline)
            .field("online", &self.online)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MachineCredentialSource {
    Stored,
    Initialized,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ResolvedMachineCredentials {
    pub credentials: MachineSandboxCredentials,
    pub source: MachineCredentialSource,
}

/// Stores the canonical sandbox account credentials for the whole machine.
///
/// Implementations must preserve a single value across Codex processes and
/// Windows user sessions. Callers serialize load-or-create operations with the
/// provisioning mutex before using the store.
pub(super) trait MachineCredentialStore {
    fn load(&mut self) -> Result<Option<MachineSandboxCredentials>>;
    fn save(&mut self, credentials: &MachineSandboxCredentials) -> Result<()>;
}

pub(super) fn load_or_create_machine_credentials(
    store: &mut impl MachineCredentialStore,
    offline_username: &str,
    online_username: &str,
    create: impl FnOnce() -> Result<MachineSandboxCredentials>,
) -> Result<ResolvedMachineCredentials> {
    if let Some(credentials) = store.load()? {
        credentials.validate(offline_username, online_username)?;
        return Ok(ResolvedMachineCredentials {
            credentials,
            source: MachineCredentialSource::Stored,
        });
    }

    let credentials = create()?;
    credentials.validate(offline_username, online_username)?;
    store.save(&credentials)?;
    Ok(ResolvedMachineCredentials {
        credentials,
        source: MachineCredentialSource::Initialized,
    })
}

pub(super) struct MachineCredentialStoreLsa {
    policy: LsaPolicy,
}

impl MachineCredentialStoreLsa {
    pub(super) fn open() -> Result<Self> {
        Ok(Self {
            policy: LsaPolicy::open(
                (POLICY_GET_PRIVATE_INFORMATION | POLICY_CREATE_SECRET) as u32,
            )?,
        })
    }
}

impl MachineCredentialStore for MachineCredentialStoreLsa {
    fn load(&mut self) -> Result<Option<MachineSandboxCredentials>> {
        let (_key_buffer, key) = lsa_unicode_string(MACHINE_CREDENTIALS_SECRET_NAME)?;
        let mut private_data: *mut LSA_UNICODE_STRING = ptr::null_mut();
        let status = unsafe { LsaRetrievePrivateData(self.policy.handle, &key, &mut private_data) };
        if status == STATUS_OBJECT_NAME_NOT_FOUND {
            return Ok(None);
        }
        if status != 0 {
            return Err(lsa_status_error("LsaRetrievePrivateData", status));
        }
        ensure!(
            !private_data.is_null(),
            "LsaRetrievePrivateData returned a null credential value"
        );

        let result = unsafe {
            let private_data_value = &*private_data;
            ensure!(
                private_data_value.Length % 2 == 0,
                "machine sandbox credential value has an invalid UTF-16 length"
            );
            let unit_count = usize::from(private_data_value.Length / 2);
            ensure!(
                unit_count == 0 || !private_data_value.Buffer.is_null(),
                "machine sandbox credential value has a null UTF-16 buffer"
            );
            let json = if unit_count == 0 {
                String::new()
            } else {
                String::from_utf16(std::slice::from_raw_parts(
                    private_data_value.Buffer,
                    unit_count,
                ))
                .context("machine sandbox credential value is not valid UTF-16")?
            };
            serde_json::from_str(&json).context("parse machine sandbox credentials")
        };
        unsafe {
            LsaFreeMemory(private_data.cast());
        }
        result.map(Some)
    }

    fn save(&mut self, credentials: &MachineSandboxCredentials) -> Result<()> {
        let json =
            serde_json::to_string(credentials).context("serialize machine sandbox credentials")?;
        let (_key_buffer, key) = lsa_unicode_string(MACHINE_CREDENTIALS_SECRET_NAME)?;
        let (_value_buffer, value) = lsa_unicode_string(&json)?;
        let status = unsafe { LsaStorePrivateData(self.policy.handle, &key, &value) };
        if status == 0 {
            Ok(())
        } else {
            Err(lsa_status_error("LsaStorePrivateData", status))
        }
    }
}

struct LsaPolicy {
    handle: LSA_HANDLE,
}

impl LsaPolicy {
    fn open(desired_access: u32) -> Result<Self> {
        let attributes = LSA_OBJECT_ATTRIBUTES {
            Length: size_of::<LSA_OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: 0,
            ObjectName: ptr::null_mut(),
            Attributes: 0,
            SecurityDescriptor: ptr::null_mut(),
            SecurityQualityOfService: ptr::null_mut(),
        };
        let mut handle: LSA_HANDLE = 0;
        let status =
            unsafe { LsaOpenPolicy(ptr::null(), &attributes, desired_access, &mut handle) };
        if status != 0 {
            return Err(lsa_status_error("LsaOpenPolicy", status));
        }
        ensure!(handle != 0, "LsaOpenPolicy returned an invalid handle");
        Ok(Self { handle })
    }
}

impl Drop for LsaPolicy {
    fn drop(&mut self) {
        unsafe {
            let _ = LsaClose(self.handle);
        }
    }
}

fn lsa_unicode_string(value: &str) -> Result<(Vec<u16>, LSA_UNICODE_STRING)> {
    let mut buffer: Vec<u16> = value.encode_utf16().collect();
    let length = u16::try_from(buffer.len().saturating_mul(size_of::<u16>()))
        .context("LSA string is too long")?;
    buffer.push(0);
    let maximum_length = u16::try_from(buffer.len().saturating_mul(size_of::<u16>()))
        .context("LSA string buffer is too long")?;
    let value = LSA_UNICODE_STRING {
        Length: length,
        MaximumLength: maximum_length,
        Buffer: buffer.as_mut_ptr(),
    };
    Ok((buffer, value))
}

fn lsa_status_error(operation: &str, status: i32) -> anyhow::Error {
    let windows_error = unsafe { LsaNtStatusToWinError(status) };
    let error = std::io::Error::from_raw_os_error(windows_error as i32);
    anyhow!("{operation} failed: {error}")
}

pub(super) struct ProvisioningMutexGuard {
    handle: isize,
}

impl Drop for ProvisioningMutexGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

pub(super) fn acquire_provisioning_mutex() -> Result<ProvisioningMutexGuard> {
    let sddl = to_wide(OsStr::new(PROVISIONING_MUTEX_SDDL));
    let mut security_descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut security_descriptor,
            ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(anyhow!(
            "ConvertStringSecurityDescriptorToSecurityDescriptorW failed for provisioning mutex: {}",
            unsafe { GetLastError() }
        ));
    }

    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security_descriptor,
        bInheritHandle: 0,
    };
    let name = to_wide(OsStr::new(PROVISIONING_MUTEX_NAME));
    let handle = unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) };
    let create_error = if handle == 0 {
        Some(unsafe { GetLastError() })
    } else {
        None
    };
    unsafe {
        LocalFree(security_descriptor as HLOCAL);
    }
    if let Some(error) = create_error {
        return Err(anyhow!(
            "CreateMutexW failed for provisioning mutex: {error}"
        ));
    }

    let wait_result = unsafe { WaitForSingleObject(handle, INFINITE) };
    match wait_result {
        WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(ProvisioningMutexGuard { handle }),
        WAIT_FAILED => {
            let error = unsafe { GetLastError() };
            unsafe {
                CloseHandle(handle);
            }
            Err(anyhow!(
                "WaitForSingleObject failed for provisioning mutex: {error}"
            ))
        }
        other => {
            unsafe {
                CloseHandle(handle);
            }
            Err(anyhow!(
                "WaitForSingleObject returned unexpected result {other} for provisioning mutex"
            ))
        }
    }
}

#[cfg(test)]
#[path = "machine_credentials_tests.rs"]
mod tests;
