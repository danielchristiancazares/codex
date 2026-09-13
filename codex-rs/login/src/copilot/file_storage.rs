//! Private, atomic credential files that do not depend on an interactive OS keyring.

use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
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

pub(super) fn delete(path: &Path) -> io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
