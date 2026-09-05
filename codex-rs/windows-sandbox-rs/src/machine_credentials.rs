//! Machine-owned sandbox credential cleanup shared with packaged uninstall.

use std::mem::size_of;
use std::ptr;

use anyhow::Result;
use anyhow::ensure;
use windows_sys::Win32::Foundation::STATUS_OBJECT_NAME_NOT_FOUND;
use windows_sys::Win32::Security::Authentication::Identity::LSA_HANDLE;
use windows_sys::Win32::Security::Authentication::Identity::LSA_OBJECT_ATTRIBUTES;
use windows_sys::Win32::Security::Authentication::Identity::LSA_UNICODE_STRING;
use windows_sys::Win32::Security::Authentication::Identity::LsaClose;
use windows_sys::Win32::Security::Authentication::Identity::LsaNtStatusToWinError;
use windows_sys::Win32::Security::Authentication::Identity::LsaOpenPolicy;
use windows_sys::Win32::Security::Authentication::Identity::LsaStorePrivateData;
use windows_sys::Win32::Security::Authentication::Identity::POLICY_CREATE_SECRET;

pub const MACHINE_SANDBOX_CREDENTIALS_SECRET_NAME: &str = "L$OpenAI.Codex.WindowsSandbox.Users.v1";

pub(crate) fn remove_machine_credentials() -> Result<()> {
    let attributes = LSA_OBJECT_ATTRIBUTES {
        Length: size_of::<LSA_OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: 0,
        ObjectName: ptr::null_mut(),
        Attributes: 0,
        SecurityDescriptor: ptr::null_mut(),
        SecurityQualityOfService: ptr::null_mut(),
    };
    let mut handle: LSA_HANDLE = 0;
    let status = unsafe {
        LsaOpenPolicy(
            ptr::null(),
            &attributes,
            POLICY_CREATE_SECRET as u32,
            &mut handle,
        )
    };
    if status != 0 {
        return Err(lsa_status_error("LsaOpenPolicy", status));
    }
    ensure!(handle != 0, "LsaOpenPolicy returned an invalid handle");
    let policy = LsaPolicy(handle);

    let mut name: Vec<u16> = MACHINE_SANDBOX_CREDENTIALS_SECRET_NAME
        .encode_utf16()
        .collect();
    let length = u16::try_from(name.len().saturating_mul(size_of::<u16>()))?;
    name.push(0);
    let maximum_length = u16::try_from(name.len().saturating_mul(size_of::<u16>()))?;
    let key = LSA_UNICODE_STRING {
        Length: length,
        MaximumLength: maximum_length,
        Buffer: name.as_mut_ptr(),
    };
    let status = unsafe { LsaStorePrivateData(policy.0, &key, ptr::null()) };
    if status == 0 || status == STATUS_OBJECT_NAME_NOT_FOUND {
        Ok(())
    } else {
        Err(lsa_status_error("LsaStorePrivateData", status))
    }
}

struct LsaPolicy(LSA_HANDLE);

impl Drop for LsaPolicy {
    fn drop(&mut self) {
        unsafe {
            let _ = LsaClose(self.0);
        }
    }
}

fn lsa_status_error(operation: &str, status: i32) -> anyhow::Error {
    let windows_error = unsafe { LsaNtStatusToWinError(status) };
    let error = std::io::Error::from_raw_os_error(windows_error as i32);
    anyhow::anyhow!("{operation} failed: {error}")
}
