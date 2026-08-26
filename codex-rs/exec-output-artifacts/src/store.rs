use crate::ArtifactAccess;
use crate::ArtifactCapture;
use crate::ArtifactCaptureStatus;
use crate::ArtifactDescriptor;
use crate::ArtifactEncoding;
use crate::ArtifactProducer;
use crate::ArtifactQueryPresentation;
use crate::ArtifactRetention;
use crate::ArtifactState;
use crate::query::RepeatedSlicePolicy;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use uuid::Uuid;

use crate::ArtifactError;
use crate::ArtifactQuery;
use crate::ArtifactQueryResult;
use crate::accounting::QuotaCharge;
use crate::accounting::QuotaLimits;
use crate::accounting::StoreAccounting;
use crate::accounting::ThreadDirectoryCharge;
use crate::accounting::accounted_file_bytes;
use crate::cleanup::cleanup_version_dir;
use crate::persistence::ARTIFACT_REF_PREFIX;
use crate::persistence::line_count;
use crate::persistence::now_unix_seconds;
use crate::persistence::parse_artifact_ref;
use crate::persistence::write_bytes_atomically;
use crate::query::apply_query;

pub(crate) const STORE_VERSION: u32 = 1;
pub(crate) const ARTIFACT_LEASE_EXTENSION: &str = "lease";
pub const DEFAULT_ARTIFACT_BYTES_CAP: usize = 16 * 1024 * 1024;
pub const DEFAULT_QUERY_BYTES_CAP: usize = 16 * 1024;
pub const DEFAULT_STORE_BYTES_CAP: u64 = 1024 * 1024 * 1024;
const MAX_PRESENTED_SLICES_PER_SCOPE: usize = 256;

#[derive(Debug, Clone)]
pub struct ArtifactStoreConfig {
    pub artifact_bytes_cap: usize,
    pub thread_bytes_cap: u64,
    pub store_bytes_cap: u64,
    pub query_bytes_cap: usize,
    pub retention: Duration,
    pub pending_retention: Duration,
}

impl Default for ArtifactStoreConfig {
    fn default() -> Self {
        Self {
            artifact_bytes_cap: DEFAULT_ARTIFACT_BYTES_CAP,
            thread_bytes_cap: 128 * 1024 * 1024,
            store_bytes_cap: DEFAULT_STORE_BYTES_CAP,
            query_bytes_cap: DEFAULT_QUERY_BYTES_CAP,
            retention: Duration::from_secs(7 * 24 * 60 * 60),
            pending_retention: Duration::from_secs(60 * 60),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub artifacts_removed: u64,
    pub bytes_removed: u64,
}

#[derive(Clone)]
pub struct ArtifactStore {
    inner: Arc<ArtifactStoreInner>,
}

struct ArtifactStoreInner {
    thread_id: String,
    version_dir: PathBuf,
    thread_dir: PathBuf,
    config: ArtifactStoreConfig,
    accounting: StoreAccounting,
    presented_slices: Mutex<PresentedSlices>,
}

#[derive(Default)]
struct PresentedSlices {
    scope: String,
    digests: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct ArtifactReservation {
    token: String,
    access: ArtifactAccess,
    descriptor: ArtifactDescriptor,
    lease: Arc<ArtifactLease>,
}

#[derive(Debug)]
struct ArtifactLease {
    // Cleanup probes this OS lock before reclaiming a pending manifest. The lock
    // is released by the kernel if the reserving process exits.
    file: Mutex<Option<fs::File>>,
    path: PathBuf,
}

pub struct SanitizedArtifactContent {
    pub bytes: Vec<u8>,
    pub media_type: String,
    pub observed_byte_count: u64,
    pub capture: ArtifactCapture,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct StoredArtifact {
    pub(crate) version: u32,
    pub(crate) created_at: i64,
    pub(crate) access: ArtifactAccess,
    pub(crate) descriptor: ArtifactDescriptor,
}

impl ArtifactStore {
    pub fn open(
        root: impl AsRef<Path>,
        thread_id: impl Into<String>,
        config: ArtifactStoreConfig,
    ) -> Result<Self, ArtifactError> {
        let thread_id = thread_id.into();
        let thread_key = format!("{:x}", Sha256::digest(thread_id.as_bytes()));
        let root = root.as_ref();
        create_private_dir(root)?;
        let version_dir = root.join("v1");
        create_private_dir(&version_dir)?;
        let thread_dir = version_dir.join(thread_key);
        let accounting = StoreAccounting::for_version_dir(version_dir.clone());
        Ok(Self {
            inner: Arc::new(ArtifactStoreInner {
                thread_id,
                version_dir,
                thread_dir,
                config,
                accounting,
                presented_slices: Mutex::new(PresentedSlices::default()),
            }),
        })
    }

    pub fn artifact_bytes_cap(&self) -> usize {
        self.inner.config.artifact_bytes_cap
    }

    pub fn reserve(
        &self,
        access: ArtifactAccess,
        producer: ArtifactProducer,
    ) -> Result<ArtifactReservation, ArtifactError> {
        if access.thread_id != self.inner.thread_id {
            return Err(ArtifactError::Unauthorized);
        }
        let token = Uuid::new_v4().simple().to_string();
        let now = now_unix_seconds();
        let expires_at = now.saturating_add(
            i64::try_from(self.inner.config.retention.as_secs()).unwrap_or(i64::MAX),
        );
        let descriptor = ArtifactDescriptor {
            artifact_ref: format!("{ARTIFACT_REF_PREFIX}{token}"),
            state: ArtifactState::Pending,
            media_type: "application/octet-stream".to_string(),
            encoding: ArtifactEncoding::Utf8,
            byte_count: 0,
            observed_byte_count: 0,
            line_count: None,
            sha256: None,
            capture: ArtifactCapture::Complete,
            producer,
            environment_id: access.environment_id.clone(),
            retention: ArtifactRetention { expires_at },
        };
        let stored = StoredArtifact {
            version: STORE_VERSION,
            created_at: now,
            access: access.clone(),
            descriptor: descriptor.clone(),
        };
        let manifest = serde_json::to_vec(&stored)?;
        let lease = self.inner.accounting.with_quota(
            &self.inner.thread_dir,
            QuotaCharge {
                file_bytes: accounted_file_bytes(manifest.len())
                    .saturating_add(accounted_file_bytes(/*contents_len*/ 0)),
                thread_directory: ThreadDirectoryCharge::CreateIfMissing,
            },
            QuotaLimits {
                thread_bytes: self.inner.config.thread_bytes_cap,
                store_bytes: self.inner.config.store_bytes_cap,
            },
            || {
                create_private_dir(&self.inner.thread_dir)?;
                let lease_path = self
                    .inner
                    .thread_dir
                    .join(format!("{token}.{ARTIFACT_LEASE_EXTENSION}"));
                let mut options = fs::OpenOptions::new();
                options.create_new(true).read(true).write(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;

                    options.mode(0o600);
                }
                let lease = Arc::new(ArtifactLease {
                    file: Mutex::new(Some(options.open(&lease_path)?)),
                    path: lease_path,
                });
                {
                    let file = lease
                        .file
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    file.as_ref()
                        .expect("new artifact lease owns its file")
                        .try_lock()
                        .map_err(|err| ArtifactError::Storage(err.into()))?;
                }
                write_bytes_atomically(&self.manifest_path(&token), &manifest)?;
                Ok(lease)
            },
        )?;
        Ok(ArtifactReservation {
            token,
            access,
            descriptor,
            lease,
        })
    }

    pub fn complete(
        &self,
        reservation: &ArtifactReservation,
        content: SanitizedArtifactContent,
    ) -> Result<ArtifactDescriptor, ArtifactError> {
        if reservation.access.thread_id != self.inner.thread_id {
            return Err(ArtifactError::Unauthorized);
        }
        if content.bytes.len() > self.inner.config.artifact_bytes_cap {
            return Err(ArtifactError::QuotaExceeded);
        }
        let added_bytes = u64::try_from(content.bytes.len()).unwrap_or(u64::MAX);
        let digest = format!("{:x}", Sha256::digest(&content.bytes));
        let line_count = line_count(&content.bytes);
        let mut descriptor = reservation.descriptor.clone();
        descriptor.state = ArtifactState::Complete;
        descriptor.media_type = content.media_type;
        descriptor.byte_count = added_bytes;
        descriptor.observed_byte_count = content.observed_byte_count;
        descriptor.line_count = Some(line_count);
        descriptor.sha256 = Some(digest);
        descriptor.capture = content.capture;
        let stored = StoredArtifact {
            version: STORE_VERSION,
            created_at: now_unix_seconds(),
            access: reservation.access.clone(),
            descriptor: descriptor.clone(),
        };
        let manifest = serde_json::to_vec(&stored)?;
        let peak_file_bytes = accounted_file_bytes(content.bytes.len())
            .saturating_add(accounted_file_bytes(manifest.len()));
        let descriptor = self.inner.accounting.with_quota(
            &self.inner.thread_dir,
            QuotaCharge {
                file_bytes: peak_file_bytes,
                thread_directory: ThreadDirectoryCharge::Existing,
            },
            QuotaLimits {
                thread_bytes: self.inner.config.thread_bytes_cap,
                store_bytes: self.inner.config.store_bytes_cap,
            },
            || {
                let stored = self.load_manifest(&reservation.token)?;
                if stored.access != reservation.access
                    || stored.access.thread_id != self.inner.thread_id
                {
                    return Err(ArtifactError::Unauthorized);
                }
                if stored.descriptor != reservation.descriptor
                    || stored.descriptor.state != ArtifactState::Pending
                {
                    return Err(ArtifactError::InvalidState);
                }
                if stored.descriptor.retention.expires_at <= now_unix_seconds() {
                    return Err(ArtifactError::Expired);
                }

                write_bytes_atomically(&self.content_path(&reservation.token), &content.bytes)?;
                if let Err(err) =
                    write_bytes_atomically(&self.manifest_path(&reservation.token), &manifest)
                {
                    let _ = fs::remove_file(self.content_path(&reservation.token));
                    return Err(err);
                }
                Ok(descriptor)
            },
        )?;
        reservation.lease.release();
        Ok(descriptor)
    }

    pub fn query(
        &self,
        artifact_ref: &str,
        access: &ArtifactAccess,
        query: &ArtifactQuery,
        presentation: &ArtifactQueryPresentation,
    ) -> Result<ArtifactQueryResult, ArtifactError> {
        let token = parse_artifact_ref(artifact_ref)?;
        let stored = self.load_manifest(&token)?;
        if stored.access != *access || access.thread_id != self.inner.thread_id {
            return Err(ArtifactError::Unauthorized);
        }
        if stored.descriptor.retention.expires_at <= now_unix_seconds() {
            return Err(ArtifactError::Expired);
        }
        if stored.descriptor.state != ArtifactState::Complete {
            return Err(ArtifactError::Incomplete);
        }
        if stored.descriptor.capture == ArtifactCapture::Truncated && query.requires_complete_tail()
        {
            return Err(ArtifactError::InvalidQuery(
                "tail is unavailable because this artifact retained only the stream prefix"
                    .to_string(),
            ));
        }
        let content = fs::read(self.content_path(&token))?;
        if !self.content_matches_descriptor(&content, &stored.descriptor) {
            return Err(ArtifactError::Corrupt);
        }
        let applied = apply_query(
            &content,
            query,
            self.inner
                .config
                .query_bytes_cap
                .min(DEFAULT_QUERY_BYTES_CAP),
        )?;
        let was_presented = applied.slice_sha256.as_ref().is_some_and(|digest| {
            let key = format!("{artifact_ref}:{digest}");
            let mut presented = self
                .inner
                .presented_slices
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if presented.scope != presentation.scope() {
                presented.scope = presentation.scope().to_string();
                presented.digests.clear();
            }
            if presented.digests.contains(&key) {
                return true;
            }
            if presented.digests.len() < MAX_PRESENTED_SLICES_PER_SCOPE {
                presented.digests.insert(key);
            }
            false
        });
        let repeated_slice =
            presentation.repeated_slice() == RepeatedSlicePolicy::ReturnReceipt && was_presented;
        Ok(ArtifactQueryResult {
            descriptor: stored.descriptor,
            data: if repeated_slice { None } else { applied.data },
            slice_sha256: applied.slice_sha256,
            repeated_slice,
        })
    }

    pub fn cleanup_expired(&self) -> Result<CleanupReport, ArtifactError> {
        self.cleanup_expired_at(now_unix_seconds())
    }

    fn cleanup_expired_at(&self, now: i64) -> Result<CleanupReport, ArtifactError> {
        self.inner
            .accounting
            .with_cleanup(|| cleanup_version_dir(&self.inner.version_dir, &self.inner.config, now))
    }

    fn load_manifest(&self, token: &str) -> Result<StoredArtifact, ArtifactError> {
        let path = self.manifest_path(token);
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(ArtifactError::NotFound);
            }
            Err(err) => return Err(ArtifactError::Storage(err)),
        };
        let stored = serde_json::from_slice::<StoredArtifact>(&bytes)?;
        if stored.version != STORE_VERSION {
            return Err(ArtifactError::Corrupt);
        }
        Ok(stored)
    }

    fn content_matches_descriptor(&self, content: &[u8], descriptor: &ArtifactDescriptor) -> bool {
        descriptor.byte_count == u64::try_from(content.len()).unwrap_or(u64::MAX)
            && descriptor
                .sha256
                .as_ref()
                .is_some_and(|expected| *expected == format!("{:x}", Sha256::digest(content)))
    }

    fn manifest_path(&self, token: &str) -> PathBuf {
        self.inner.thread_dir.join(format!("{token}.json"))
    }

    fn content_path(&self, token: &str) -> PathBuf {
        self.inner.thread_dir.join(format!("{token}.data"))
    }
}

impl Drop for ArtifactLease {
    fn drop(&mut self) {
        self.release();
    }
}

impl ArtifactLease {
    fn release(&self) {
        let file = self
            .file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        drop(file);
        let _ = fs::remove_file(&self.path);
    }
}

fn create_private_dir(path: &Path) -> Result<(), ArtifactError> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

impl ArtifactReservation {
    pub fn descriptor(&self) -> &ArtifactDescriptor {
        &self.descriptor
    }

    pub fn pending_descriptor(&self, status: ArtifactCaptureStatus) -> ArtifactDescriptor {
        let mut descriptor = self.descriptor.clone();
        descriptor.byte_count = status.retained_byte_count;
        descriptor.observed_byte_count = status.observed_byte_count;
        descriptor.capture = status.capture;
        descriptor
    }
}
