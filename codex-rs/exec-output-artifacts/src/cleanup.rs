use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::ArtifactError;
use crate::ArtifactState;
use crate::store::ARTIFACT_LEASE_EXTENSION;
use crate::store::ArtifactStoreConfig;
use crate::store::CleanupReport;
use crate::store::STORE_VERSION;
use crate::store::StoredArtifact;

pub(crate) fn cleanup_version_dir(
    version_dir: &Path,
    config: &ArtifactStoreConfig,
    now: i64,
) -> Result<CleanupReport, ArtifactError> {
    let mut report = CleanupReport::default();
    for entry in fs::read_dir(version_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            cleanup_thread_dir(&entry.path(), config, now, &mut report)?;
        }
    }
    Ok(report)
}

fn cleanup_thread_dir(
    thread_dir: &Path,
    config: &ArtifactStoreConfig,
    now: i64,
    report: &mut CleanupReport,
) -> Result<(), ArtifactError> {
    let mut retained_tokens = HashSet::new();
    for entry in fs::read_dir(thread_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file()
            || path.extension().and_then(std::ffi::OsStr::to_str) != Some("json")
        {
            continue;
        }
        let Some(token) = path
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .filter(|token| {
                token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        else {
            if path_is_stale(&path, now, config.pending_retention) {
                fs::remove_file(&path)?;
            }
            continue;
        };
        let read_stored = || {
            fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<StoredArtifact>(&bytes).ok())
        };
        let stored = read_stored();
        let should_remove = |stored: Option<&StoredArtifact>| {
            let remove_now = stored.is_some_and(|stored| {
                let pending_expiry = stored.created_at.saturating_add(
                    i64::try_from(config.pending_retention.as_secs()).unwrap_or(i64::MAX),
                );
                stored.version == STORE_VERSION
                    && (stored.descriptor.retention.expires_at <= now
                        || (stored.descriptor.state == ArtifactState::Pending
                            && pending_expiry <= now))
            });
            let remove_if_stale = stored.is_none_or(|stored| stored.version != STORE_VERSION);
            remove_now || (remove_if_stale && path_is_stale(&path, now, config.pending_retention))
        };
        let remove_candidate = should_remove(stored.as_ref());
        let requires_inactive_lease = stored.as_ref().is_none_or(|stored| {
            stored.version != STORE_VERSION || stored.descriptor.state == ArtifactState::Pending
        });
        let mut reclamation_lease = None;
        let can_remove = if remove_candidate && requires_inactive_lease {
            let lease_path = thread_dir.join(format!("{token}.{ARTIFACT_LEASE_EXTENSION}"));
            match fs::File::options().read(true).write(true).open(lease_path) {
                Ok(file) => match file.try_lock() {
                    Ok(()) => {
                        // The owner may have completed the artifact between the
                        // first manifest read and this lock acquisition.
                        let latest_stored = read_stored();
                        let can_remove = should_remove(latest_stored.as_ref());
                        if can_remove {
                            reclamation_lease = Some(file);
                        }
                        can_remove
                    }
                    Err(fs::TryLockError::WouldBlock) => false,
                    Err(err) => return Err(ArtifactError::Storage(err.into())),
                },
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    should_remove(read_stored().as_ref())
                }
                Err(err) => return Err(ArtifactError::Storage(err)),
            }
        } else {
            remove_candidate
        };
        if can_remove {
            remove_artifact_files(thread_dir, token, &path, report)?;
            drop(reclamation_lease);
        } else {
            retained_tokens.insert(token.to_string());
        }
    }

    for entry in fs::read_dir(thread_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() || !path_is_stale(&path, now, config.pending_retention) {
            continue;
        }
        let extension = path.extension().and_then(std::ffi::OsStr::to_str);
        match extension {
            Some("data")
                if path
                    .file_stem()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_none_or(|token| !retained_tokens.contains(token)) =>
            {
                let bytes = entry.metadata()?.len();
                fs::remove_file(&path)?;
                report.artifacts_removed = report.artifacts_removed.saturating_add(1);
                report.bytes_removed = report.bytes_removed.saturating_add(bytes);
            }
            Some("tmp") => {
                let bytes = entry.metadata()?.len();
                fs::remove_file(&path)?;
                report.artifacts_removed = report.artifacts_removed.saturating_add(1);
                report.bytes_removed = report.bytes_removed.saturating_add(bytes);
            }
            Some(ARTIFACT_LEASE_EXTENSION) => {
                let file = match fs::File::options().read(true).write(true).open(&path) {
                    Ok(file) => file,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(err) => return Err(ArtifactError::Storage(err)),
                };
                match file.try_lock() {
                    Ok(()) => match fs::remove_file(&path) {
                        Ok(()) => {}
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                        Err(err) => return Err(ArtifactError::Storage(err)),
                    },
                    Err(fs::TryLockError::WouldBlock) => {}
                    Err(err) => return Err(ArtifactError::Storage(err.into())),
                }
            }
            Some(_) | None => {}
        }
    }
    if fs::read_dir(thread_dir)?.next().transpose()?.is_none() {
        fs::remove_dir(thread_dir)?;
    }
    Ok(())
}

fn remove_artifact_files(
    thread_dir: &Path,
    token: &str,
    manifest_path: &Path,
    report: &mut CleanupReport,
) -> Result<(), ArtifactError> {
    let content_path = thread_dir.join(format!("{token}.data"));
    let content_len = fs::metadata(&content_path).map_or(0, |metadata| metadata.len());
    match fs::remove_file(&content_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(ArtifactError::Storage(err)),
    }
    fs::remove_file(manifest_path)?;
    report.artifacts_removed = report.artifacts_removed.saturating_add(1);
    report.bytes_removed = report.bytes_removed.saturating_add(content_len);
    Ok(())
}

fn path_is_stale(path: &Path, now: i64, retention: Duration) -> bool {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let modified = modified.duration_since(UNIX_EPOCH).map_or(0, |duration| {
        i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
    });
    modified.saturating_add(i64::try_from(retention.as_secs()).unwrap_or(i64::MAX)) <= now
}
