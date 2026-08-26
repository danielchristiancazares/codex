use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::Weak;

use crate::ArtifactError;

const LOCK_FILE_NAME: &str = ".quota.lock";
// Charge every filesystem object for content plus a conservative metadata and
// allocation allowance. This gives empty files and directories finite quota
// cost and therefore places a hard bound on store-owned filesystem entries.
const FILESYSTEM_ENTRY_BYTES: u64 = 4 * 1024;

static ROOT_ACCOUNTING: LazyLock<Mutex<HashMap<PathBuf, Weak<RootAccounting>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
pub(crate) struct StoreAccounting {
    root: Arc<RootAccounting>,
}

struct RootAccounting {
    version_dir: PathBuf,
    process_lock: Mutex<()>,
}

struct AccountingState {
    store_bytes: u64,
    thread_bytes: HashMap<PathBuf, u64>,
}

pub(crate) struct QuotaLimits {
    pub thread_bytes: u64,
    pub store_bytes: u64,
}

pub(crate) struct QuotaCharge {
    pub file_bytes: u64,
    pub thread_directory: ThreadDirectoryCharge,
}

pub(crate) enum ThreadDirectoryCharge {
    Existing,
    CreateIfMissing,
}

impl StoreAccounting {
    pub(crate) fn for_version_dir(version_dir: PathBuf) -> Self {
        let mut roots = ROOT_ACCOUNTING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        roots.retain(|_, accounting| accounting.strong_count() > 0);
        let root = roots
            .get(&version_dir)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let root = Arc::new(RootAccounting {
                    version_dir: version_dir.clone(),
                    process_lock: Mutex::new(()),
                });
                roots.insert(version_dir, Arc::downgrade(&root));
                root
            });
        Self { root }
    }

    pub(crate) fn with_quota<T>(
        &self,
        thread_dir: &Path,
        charge: QuotaCharge,
        limits: QuotaLimits,
        operation: impl FnOnce() -> Result<T, ArtifactError>,
    ) -> Result<T, ArtifactError> {
        self.with_store_lock(|| {
            let state = scan(&self.root.version_dir)?;
            let thread_bytes = state.thread_bytes.get(thread_dir).copied();
            let directory_bytes = match charge.thread_directory {
                ThreadDirectoryCharge::Existing => 0,
                ThreadDirectoryCharge::CreateIfMissing if thread_bytes.is_none() => {
                    FILESYSTEM_ENTRY_BYTES
                }
                ThreadDirectoryCharge::CreateIfMissing => 0,
            };
            let added_bytes = charge.file_bytes.saturating_add(directory_bytes);
            if thread_bytes.unwrap_or_default().saturating_add(added_bytes) > limits.thread_bytes
                || state.store_bytes.saturating_add(added_bytes) > limits.store_bytes
            {
                return Err(ArtifactError::QuotaExceeded);
            }

            operation()
        })
    }

    pub(crate) fn with_cleanup<T>(
        &self,
        operation: impl FnOnce() -> Result<T, ArtifactError>,
    ) -> Result<T, ArtifactError> {
        self.with_store_lock(operation)
    }

    fn with_store_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, ArtifactError>,
    ) -> Result<T, ArtifactError> {
        let _process_guard = self
            .root
            .process_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let lock_file = open_lock_file(&self.root.version_dir.join(LOCK_FILE_NAME))?;
        lock_file.lock()?;
        operation()
    }
}

pub(crate) fn accounted_file_bytes(contents_len: usize) -> u64 {
    u64::try_from(contents_len)
        .unwrap_or(u64::MAX)
        .saturating_add(FILESYSTEM_ENTRY_BYTES)
}

fn open_lock_file(path: &Path) -> Result<File, ArtifactError> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

fn scan(version_dir: &Path) -> Result<AccountingState, ArtifactError> {
    let mut store_bytes = FILESYSTEM_ENTRY_BYTES;
    let mut thread_bytes = HashMap::new();
    for entry in fs::read_dir(version_dir)? {
        let entry = entry?;
        let entry_bytes = entry_bytes(&entry.path())?;
        store_bytes = store_bytes.saturating_add(entry_bytes);
        if entry.file_type()?.is_dir() {
            thread_bytes.insert(entry.path(), entry_bytes);
        }
    }
    Ok(AccountingState {
        store_bytes,
        thread_bytes,
    })
}

fn entry_bytes(path: &Path) -> Result<u64, ArtifactError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(ArtifactError::Storage(err)),
    };
    if !metadata.file_type().is_dir() {
        return Ok(metadata.len().saturating_add(FILESYSTEM_ENTRY_BYTES));
    }

    let mut total = FILESYSTEM_ENTRY_BYTES;
    for entry in fs::read_dir(path)? {
        total = total.saturating_add(entry_bytes(&entry?.path())?);
    }
    Ok(total)
}
