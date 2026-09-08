//! Private, atomic credential files that do not depend on an interactive OS keyring.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const MAX_CREDENTIAL_BYTES: u64 = 128 * 1024;

pub(super) fn load(path: &Path) -> io::Result<Option<String>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut encoded = String::new();
    file.take(MAX_CREDENTIAL_BYTES + 1)
        .read_to_string(&mut encoded)?;
    if encoded.len() as u64 > MAX_CREDENTIAL_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GitHub Copilot credential file exceeds its size limit",
        ));
    }
    Ok(Some(encoded))
}

pub(super) fn save(path: &Path, encoded: &str) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "credential file has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let nonce = rand::random::<u64>();
    let temporary_path = parent.join(format!(".copilot-auth-{nonce:016x}.tmp"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary_path)?;
    let result = (|| {
        file.write_all(encoded.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

pub(super) fn delete(path: &Path) -> io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
