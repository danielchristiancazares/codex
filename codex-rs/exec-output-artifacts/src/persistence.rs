use crate::ArtifactError;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use uuid::Uuid;

pub(crate) const ARTIFACT_REF_PREFIX: &str = "exec-output-artifact://v1/";

pub(crate) fn parse_artifact_ref(artifact_ref: &str) -> Result<String, ArtifactError> {
    let token = artifact_ref
        .strip_prefix(ARTIFACT_REF_PREFIX)
        .ok_or(ArtifactError::InvalidReference)?;
    if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ArtifactError::InvalidReference);
    }
    Ok(token.to_ascii_lowercase())
}

pub(crate) fn line_count(bytes: &[u8]) -> u64 {
    if bytes.is_empty() {
        return 0;
    }
    let newlines = bytes.iter().filter(|byte| **byte == b'\n').count();
    u64::try_from(newlines + usize::from(!bytes.ends_with(b"\n"))).unwrap_or(u64::MAX)
}

pub(crate) fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

pub(crate) fn write_bytes_atomically(path: &Path, contents: &[u8]) -> Result<(), ArtifactError> {
    let parent = path.parent().ok_or_else(|| {
        ArtifactError::Storage(std::io::Error::other("artifact path has no parent"))
    })?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| {
            ArtifactError::Storage(std::io::Error::other("artifact path has no filename"))
        })?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4().simple()));
    {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    if let Err(initial_error) = publish_file(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(ArtifactError::Storage(initial_error));
    }
    Ok(())
}

#[cfg(not(windows))]
fn publish_file(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn publish_file(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING;
    use windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both paths are NUL-terminated UTF-16 buffers that remain alive
    // for the call, and the temporary file lives beside the destination.
    let result = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
