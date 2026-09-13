use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use serde::de::DeserializeOwned;

pub(super) fn read<T: DeserializeOwned>(path: &Path) -> std::io::Result<T> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 {
        return Err(std::io::Error::other(
            "Connection metadata exceeds its size limit.",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| std::io::Error::other("Invalid connection metadata."))
}

/// Cross-process authority over one credential scope's refresh sequence.
pub(crate) struct RefreshLease {
    _file: File,
}

pub(super) fn remove_registration(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

impl RefreshLease {
    pub(crate) async fn acquire(home: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&home)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(home.join(".credential-refresh.lock"))?;
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match file.try_lock() {
                    Ok(()) => return Ok(Self { _file: file }),
                    Err(std::fs::TryLockError::WouldBlock) => {
                        tokio::time::sleep(Duration::from_millis(50)).await
                    }
                    Err(std::fs::TryLockError::Error(error)) => return Err(error),
                }
            }
        })
        .await
        .map_err(|_| std::io::Error::other("Timed out waiting for another account refresh."))?
    }
}
