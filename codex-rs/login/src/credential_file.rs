//! Publishes private credential files and removes incomplete writes on failure.

use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    write(path, &serde_json::to_vec(value)?)
}

pub(crate) fn write(path: &Path, encoded: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "credential file has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let nonce = rand::random::<u128>();
    let temporary_path = parent.join(format!(".credential-{nonce:032x}.tmp"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary_path)?;
    let result = (|| {
        file.write_all(encoded)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

#[cfg(test)]
#[path = "credential_file_tests.rs"]
mod tests;
