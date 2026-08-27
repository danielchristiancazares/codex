use crate::ArtifactCapture;
use crate::ArtifactProducer;
use crate::ArtifactProducerKind;
use crate::ArtifactState;
use crate::ArtifactStream;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use std::time::Duration;

use super::*;

const QUOTA_TEST_CHILD_ENV: &str = "CODEX_EXEC_OUTPUT_ARTIFACTS_QUOTA_TEST_CHILD";
const QUOTA_TEST_ROOT_ENV: &str = "CODEX_EXEC_OUTPUT_ARTIFACTS_QUOTA_TEST_ROOT";
const LEASE_TEST_CHILD_ENV: &str = "CODEX_EXEC_OUTPUT_ARTIFACTS_LEASE_TEST_CHILD";
const LEASE_TEST_ROOT_ENV: &str = "CODEX_EXEC_OUTPUT_ARTIFACTS_LEASE_TEST_ROOT";

fn access(thread_id: &str, root: &str) -> ArtifactAccess {
    ArtifactAccess::new(thread_id, "local", [root])
}

fn producer(stream: ArtifactStream) -> ArtifactProducer {
    ArtifactProducer {
        kind: ArtifactProducerKind::ProcessStream,
        item_id: "call-1".to_string(),
        process_id: Some("1000".to_string()),
        stream,
    }
}

fn content(text: &str) -> SanitizedArtifactContent {
    SanitizedArtifactContent {
        bytes: text.as_bytes().to_vec(),
        media_type: "text/plain".to_string(),
        observed_byte_count: u64::try_from(text.len()).expect("test text length"),
        capture: ArtifactCapture::Complete,
    }
}

fn store(root: &std::path::Path, thread_id: &str) -> Result<ArtifactStore, ArtifactError> {
    ArtifactStore::open(root, thread_id, ArtifactStoreConfig::default())
}

fn presentation(scope: &str) -> ArtifactQueryPresentation {
    ArtifactQueryPresentation::return_receipt(scope)
}

fn wait_for_path(path: &std::path::Path) -> Result<(), ArtifactError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        if std::time::Instant::now() >= deadline {
            return Err(ArtifactError::Storage(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("timed out waiting for {}", path.display()),
            )));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn expire_manifest(
    root: &std::path::Path,
    thread_id: &str,
    artifact_ref: &str,
) -> Result<(), ArtifactError> {
    let thread_key = format!("{:x}", Sha256::digest(thread_id.as_bytes()));
    let token = artifact_ref
        .strip_prefix("exec-output-artifact://v1/")
        .expect("known artifact reference");
    let path = root
        .join("v1")
        .join(thread_key)
        .join(format!("{token}.json"));
    let mut manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    manifest["descriptor"]["retention"]["expires_at"] = serde_json::json!(0);
    std::fs::write(path, serde_json::to_vec(&manifest)?)?;
    Ok(())
}

#[test]
fn descriptors_and_queries_round_trip_complete_content() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let store = store(temp.path(), "thread-a")?;
    let access = access("thread-a", "file:///workspace");
    let reservation = store.reserve(access.clone(), producer(ArtifactStream::Stdout))?;
    let descriptor = store.complete(
        &reservation,
        content("alpha\nneedle one\nbeta\nneedle two\nomega\n"),
    )?;

    assert_eq!(descriptor.state, ArtifactState::Complete);
    assert_eq!(descriptor.byte_count, 39);
    assert_eq!(descriptor.line_count, Some(5));
    assert_eq!(descriptor.capture, ArtifactCapture::Complete);

    let head = store.query(
        &descriptor.artifact_ref,
        &access,
        &ArtifactQuery::Head { max_bytes: 5 },
        &presentation("turn-1:0"),
    )?;
    assert_eq!(
        head.data,
        Some(ArtifactQueryData::Text {
            text: "alpha".to_string(),
            byte_start: 0,
            byte_end: 5,
            line_start: Some(1),
            line_end: Some(1),
            truncated: true,
        })
    );

    let tail = store.query(
        &descriptor.artifact_ref,
        &access,
        &ArtifactQuery::Tail { max_bytes: 6 },
        &presentation("turn-1:0"),
    )?;
    assert_eq!(
        tail.data,
        Some(ArtifactQueryData::Text {
            text: "omega\n".to_string(),
            byte_start: 33,
            byte_end: 39,
            line_start: Some(5),
            line_end: Some(5),
            truncated: true,
        })
    );

    let lines = store.query(
        &descriptor.artifact_ref,
        &access,
        &ArtifactQuery::Lines {
            start: 2,
            end: 3,
            max_bytes: 128,
        },
        &presentation("turn-1:0"),
    )?;
    assert_eq!(
        lines.data,
        Some(ArtifactQueryData::Text {
            text: "needle one\nbeta\n".to_string(),
            byte_start: 6,
            byte_end: 22,
            line_start: Some(2),
            line_end: Some(3),
            truncated: true,
        })
    );

    let literal = store.query(
        &descriptor.artifact_ref,
        &access,
        &ArtifactQuery::Search {
            pattern: "needle".to_string(),
            mode: ArtifactSearchMode::Literal,
            case_sensitive: true,
            context_lines: 0,
            max_matches: 10,
            max_bytes: 1024,
        },
        &presentation("turn-1:0"),
    )?;
    let Some(ArtifactQueryData::Matches { matches, truncated }) = literal.data else {
        panic!("expected search matches");
    };
    assert_eq!(
        matches,
        vec![
            ArtifactSearchMatch {
                line: 2,
                byte_start: 6,
                byte_end: 17,
                text: "needle one\n".to_string(),
            },
            ArtifactSearchMatch {
                line: 4,
                byte_start: 22,
                byte_end: 33,
                text: "needle two\n".to_string(),
            },
        ]
    );
    assert!(!truncated);

    Ok(())
}

#[test]
fn byte_ranges_reconstruct_exact_sanitized_content() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let store = store(temp.path(), "thread-a")?;
    let access = access("thread-a", "file:///workspace");
    let expected = "first\nsecond\nthird\n";
    let reservation = store.reserve(access.clone(), producer(ArtifactStream::Stdout))?;
    let descriptor = store.complete(&reservation, content(expected))?;
    let mut reconstructed = Vec::new();

    for start in (0..expected.len()).step_by(4) {
        let result = store.query(
            &descriptor.artifact_ref,
            &access,
            &ArtifactQuery::Bytes { start, length: 4 },
            &presentation("turn-1:0"),
        )?;
        let Some(ArtifactQueryData::Bytes { data_base64, .. }) = result.data else {
            panic!("expected byte-range data");
        };
        reconstructed.extend(STANDARD.decode(data_base64).expect("valid base64"));
    }

    assert_eq!(reconstructed, expected.as_bytes());
    Ok(())
}

#[test]
fn repeated_query_returns_a_digest_receipt() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let store = store(temp.path(), "thread-a")?;
    let access = access("thread-a", "file:///workspace");
    let reservation = store.reserve(access.clone(), producer(ArtifactStream::Stdout))?;
    let descriptor = store.complete(&reservation, content("same slice"))?;
    let query = ArtifactQuery::Head { max_bytes: 32 };

    let first = store.query(
        &descriptor.artifact_ref,
        &access,
        &query,
        &presentation("turn-1:0"),
    )?;
    let repeated = store.query(
        &descriptor.artifact_ref,
        &access,
        &query,
        &presentation("turn-1:0"),
    )?;
    let after_compaction = store.query(
        &descriptor.artifact_ref,
        &access,
        &query,
        &presentation("turn-1:1"),
    )?;
    let forced = store.query(
        &descriptor.artifact_ref,
        &access,
        &query,
        &ArtifactQueryPresentation::include_data("turn-1:1"),
    )?;

    assert!(first.data.is_some());
    assert!(!first.repeated_slice);
    assert_eq!(repeated.data, None);
    assert!(repeated.repeated_slice);
    assert_eq!(repeated.slice_sha256, first.slice_sha256);
    assert!(after_compaction.data.is_some());
    assert!(!after_compaction.repeated_slice);
    assert!(forced.data.is_some());
    assert!(!forced.repeated_slice);
    Ok(())
}

#[test]
fn capture_buffer_reports_bounded_truncation() {
    let capture = ArtifactCaptureBuffer::new(/*byte_cap*/ 5);
    capture.push(b"abc");
    capture.push(b"defgh");

    assert_eq!(
        capture.status(),
        ArtifactCaptureStatus {
            retained_byte_count: 5,
            observed_byte_count: 8,
            capture: ArtifactCapture::Truncated,
        }
    );
    assert_eq!(
        capture.take(),
        ArtifactCaptureSnapshot {
            bytes: b"abcde".to_vec(),
            observed_byte_count: 8,
            capture: ArtifactCapture::Truncated,
        }
    );
    assert_eq!(
        capture.status(),
        ArtifactCaptureStatus {
            retained_byte_count: 0,
            observed_byte_count: 8,
            capture: ArtifactCapture::Truncated,
        }
    );
}

#[test]
fn truncated_prefix_does_not_claim_to_provide_the_stream_tail() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let store = store(temp.path(), "thread-a")?;
    let access = access("thread-a", "file:///workspace");
    let reservation = store.reserve(access.clone(), producer(ArtifactStream::Stdout))?;
    let mut captured_prefix = content("prefix");
    captured_prefix.observed_byte_count = 64;
    captured_prefix.capture = ArtifactCapture::Truncated;
    let descriptor = store.complete(&reservation, captured_prefix)?;

    assert!(matches!(
        store.query(
            &descriptor.artifact_ref,
            &access,
            &ArtifactQuery::Tail { max_bytes: 6 },
            &presentation("turn-1:0"),
        ),
        Err(ArtifactError::InvalidQuery(message))
            if message.contains("retained only the stream prefix")
    ));
    Ok(())
}

#[test]
fn access_is_bound_to_thread_and_workspace_authority() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let store_a = store(temp.path(), "thread-a")?;
    let store_b = store(temp.path(), "thread-b")?;
    let owner = access("thread-a", "file:///workspace-a");
    let reservation = store_a.reserve(owner, producer(ArtifactStream::Stdout))?;
    let descriptor = store_a.complete(&reservation, content("private"))?;

    let wrong_workspace = access("thread-a", "file:///workspace-b");
    assert!(matches!(
        store_a.query(
            &descriptor.artifact_ref,
            &wrong_workspace,
            &ArtifactQuery::Metadata,
            &presentation("turn-1:0"),
        ),
        Err(ArtifactError::Unauthorized)
    ));
    assert!(matches!(
        store_b.query(
            &descriptor.artifact_ref,
            &access("thread-b", "file:///workspace-a"),
            &ArtifactQuery::Metadata,
            &presentation("turn-1:0"),
        ),
        Err(ArtifactError::NotFound)
    ));
    Ok(())
}

#[test]
fn pending_expired_and_corrupt_artifacts_fail_closed() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let owner_access = access("thread-a", "file:///workspace");
    let expiring = ArtifactStore::open(
        temp.path(),
        "thread-a",
        ArtifactStoreConfig {
            retention: Duration::ZERO,
            ..ArtifactStoreConfig::default()
        },
    )?;
    let reservation = expiring.reserve(owner_access.clone(), producer(ArtifactStream::Stdout))?;
    assert!(matches!(
        expiring.complete(&reservation, content("expired")),
        Err(ArtifactError::Expired)
    ));
    assert!(matches!(
        expiring.query(
            &reservation.descriptor().artifact_ref,
            &owner_access,
            &ArtifactQuery::Metadata,
            &presentation("turn-1:0"),
        ),
        Err(ArtifactError::Expired)
    ));

    let pending_store = store(temp.path(), "thread-pending")?;
    let pending_access = access("thread-pending", "file:///workspace");
    let pending =
        pending_store.reserve(pending_access.clone(), producer(ArtifactStream::Stderr))?;
    assert!(matches!(
        pending_store.query(
            &pending.descriptor().artifact_ref,
            &pending_access,
            &ArtifactQuery::Metadata,
            &presentation("turn-1:0"),
        ),
        Err(ArtifactError::Incomplete)
    ));

    let corrupt_store = store(temp.path(), "thread-corrupt")?;
    let corrupt_access = access("thread-corrupt", "file:///workspace");
    let corrupt =
        corrupt_store.reserve(corrupt_access.clone(), producer(ArtifactStream::Stdout))?;
    let corrupt_descriptor = corrupt_store.complete(&corrupt, content("intact"))?;
    let thread_key = format!("{:x}", Sha256::digest(b"thread-corrupt"));
    let token = corrupt_descriptor
        .artifact_ref
        .strip_prefix("exec-output-artifact://v1/")
        .expect("known artifact reference");
    std::fs::write(
        temp.path()
            .join("v1")
            .join(thread_key)
            .join(format!("{token}.data")),
        "tampered",
    )?;
    assert!(matches!(
        corrupt_store.query(
            &corrupt_descriptor.artifact_ref,
            &corrupt_access,
            &ArtifactQuery::Metadata,
            &presentation("turn-1:0"),
        ),
        Err(ArtifactError::Corrupt)
    ));
    Ok(())
}

#[test]
fn cleanup_removes_expired_content_and_metadata() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let access = access("thread-a", "file:///workspace");
    let store = store(temp.path(), "thread-a")?;
    let reservation = store.reserve(access, producer(ArtifactStream::Stdout))?;
    let descriptor = store.complete(&reservation, content("remove me"))?;
    expire_manifest(temp.path(), "thread-a", &descriptor.artifact_ref)?;

    let report = store.cleanup_expired()?;
    assert_eq!(
        report,
        CleanupReport {
            artifacts_removed: 1,
            bytes_removed: 9,
        }
    );
    Ok(())
}

#[test]
fn cleanup_preserves_a_live_pending_reservation_across_processes() -> Result<(), ArtifactError> {
    if std::env::var_os(LEASE_TEST_CHILD_ENV).is_some() {
        let root = std::path::PathBuf::from(
            std::env::var_os(LEASE_TEST_ROOT_ENV).expect("lease test child has a shared root"),
        );
        let owner = store(&root, "thread-owner")?;
        let reservation = owner.reserve(
            access("thread-owner", "file:///workspace"),
            producer(ArtifactStream::Stdout),
        )?;
        std::fs::write(root.join("lease-ready"), b"ready")?;
        wait_for_path(&root.join("lease-go"))?;
        owner.complete(&reservation, content("long-running output"))?;
        return Ok(());
    }

    let temp = tempfile::tempdir()?;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg("tests::cleanup_preserves_a_live_pending_reservation_across_processes")
        .arg("--nocapture")
        .env(LEASE_TEST_CHILD_ENV, "1")
        .env(LEASE_TEST_ROOT_ENV, temp.path())
        .spawn()?;
    wait_for_path(&temp.path().join("lease-ready"))?;
    let cleaner = ArtifactStore::open(
        temp.path(),
        "thread-cleaner",
        ArtifactStoreConfig {
            pending_retention: Duration::ZERO,
            ..ArtifactStoreConfig::default()
        },
    )?;
    let report = cleaner.cleanup_expired()?;
    std::fs::write(temp.path().join("lease-go"), b"go")?;
    let status = child.wait()?;

    assert_eq!(report, CleanupReport::default());
    assert!(status.success());
    Ok(())
}

#[test]
fn cleanup_reclaims_an_abandoned_pending_reservation() -> Result<(), ArtifactError> {
    if std::env::var_os(LEASE_TEST_CHILD_ENV).is_some() {
        let root = std::path::PathBuf::from(
            std::env::var_os(LEASE_TEST_ROOT_ENV).expect("lease test child has a shared root"),
        );
        let owner = store(&root, "thread-owner")?;
        let reservation = owner.reserve(
            access("thread-owner", "file:///workspace"),
            producer(ArtifactStream::Stderr),
        )?;
        std::fs::write(
            root.join("abandoned-lease-ready"),
            reservation.descriptor().artifact_ref.as_bytes(),
        )?;
        // Leave the lease file behind so process exit, rather than Drop,
        // releases its OS lock.
        std::mem::forget(reservation);
        return Ok(());
    }

    let temp = tempfile::tempdir()?;
    let owner_access = access("thread-owner", "file:///workspace");
    let owner = store(temp.path(), "thread-owner")?;
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg("tests::cleanup_reclaims_an_abandoned_pending_reservation")
        .arg("--nocapture")
        .env(LEASE_TEST_CHILD_ENV, "1")
        .env(LEASE_TEST_ROOT_ENV, temp.path())
        .spawn()?;
    let ready_path = temp.path().join("abandoned-lease-ready");
    wait_for_path(&ready_path)?;
    assert!(child.wait()?.success());
    let artifact_ref = std::fs::read_to_string(ready_path)?;
    let cleaner = ArtifactStore::open(
        temp.path(),
        "thread-cleaner",
        ArtifactStoreConfig {
            pending_retention: Duration::ZERO,
            ..ArtifactStoreConfig::default()
        },
    )?;

    assert_eq!(
        cleaner.cleanup_expired()?,
        CleanupReport {
            artifacts_removed: 1,
            bytes_removed: 0,
        }
    );
    assert!(matches!(
        owner.query(
            &artifact_ref,
            &owner_access,
            &ArtifactQuery::Metadata,
            &presentation("turn-1:0"),
        ),
        Err(ArtifactError::NotFound)
    ));
    Ok(())
}

#[test]
fn opening_threads_does_not_create_unmetered_directories() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;

    for index in 0..100 {
        ArtifactStore::open(
            temp.path(),
            format!("thread-{index}"),
            ArtifactStoreConfig::default(),
        )?;
    }

    assert_eq!(std::fs::read_dir(temp.path().join("v1"))?.count(), 0);
    Ok(())
}

#[test]
fn empty_artifacts_consume_quota() -> Result<(), ArtifactError> {
    let temp = tempfile::tempdir()?;
    let config = ArtifactStoreConfig {
        artifact_bytes_cap: 0,
        thread_bytes_cap: 64 * 1024,
        store_bytes_cap: 64 * 1024,
        ..ArtifactStoreConfig::default()
    };
    let store = ArtifactStore::open(temp.path(), "thread-a", config)?;
    let owner_access = access("thread-a", "file:///workspace");
    let mut completed = 0;
    let mut quota_exceeded = false;

    for _ in 0..100 {
        let reservation =
            match store.reserve(owner_access.clone(), producer(ArtifactStream::Stdout)) {
                Ok(reservation) => reservation,
                Err(ArtifactError::QuotaExceeded) => {
                    quota_exceeded = true;
                    break;
                }
                Err(err) => return Err(err),
            };
        match store.complete(&reservation, content("")) {
            Ok(_) => completed += 1,
            Err(ArtifactError::QuotaExceeded) => {
                quota_exceeded = true;
                break;
            }
            Err(err) => return Err(err),
        }
    }

    assert!(completed > 0);
    assert!(quota_exceeded);
    Ok(())
}

#[test]
fn store_quota_is_atomic_across_processes() -> Result<(), ArtifactError> {
    if let Some(child_id) = std::env::var_os(QUOTA_TEST_CHILD_ENV) {
        let root = std::path::PathBuf::from(
            std::env::var_os(QUOTA_TEST_ROOT_ENV).expect("quota test child has a shared root"),
        );
        let child_id = child_id.to_string_lossy();
        let thread_id = format!("thread-{child_id}");
        let store = ArtifactStore::open(
            &root,
            &thread_id,
            ArtifactStoreConfig {
                artifact_bytes_cap: 768 * 1024,
                thread_bytes_cap: 900 * 1024,
                store_bytes_cap: 1024 * 1024,
                ..ArtifactStoreConfig::default()
            },
        )?;
        let owner_access = access(&thread_id, "file:///workspace");
        let seed = store.reserve(owner_access.clone(), producer(ArtifactStream::Stdout))?;
        store.complete(&seed, content(""))?;
        drop(seed);
        let reservation = store.reserve(owner_access, producer(ArtifactStream::Stderr))?;
        std::fs::write(root.join(format!("ready-{child_id}")), b"ready")?;
        wait_for_path(&root.join("go"))?;

        let outcome = match store.complete(
            &reservation,
            SanitizedArtifactContent {
                bytes: vec![b'x'; 700 * 1024],
                media_type: "text/plain".to_string(),
                observed_byte_count: 700 * 1024,
                capture: ArtifactCapture::Complete,
            },
        ) {
            Ok(_) => "committed",
            Err(ArtifactError::QuotaExceeded) => "quota_exceeded",
            Err(err) => return Err(err),
        };
        std::fs::write(root.join(format!("result-{child_id}")), outcome)?;
        return Ok(());
    }

    let temp = tempfile::tempdir()?;
    let executable = std::env::current_exe()?;
    let mut children = Vec::new();
    for child_id in ["a", "b"] {
        children.push(
            std::process::Command::new(&executable)
                .arg("--exact")
                .arg("tests::store_quota_is_atomic_across_processes")
                .arg("--nocapture")
                .env(QUOTA_TEST_CHILD_ENV, child_id)
                .env(QUOTA_TEST_ROOT_ENV, temp.path())
                .spawn()?,
        );
    }
    wait_for_path(&temp.path().join("ready-a"))?;
    wait_for_path(&temp.path().join("ready-b"))?;
    std::fs::write(temp.path().join("go"), b"go")?;

    for child in &mut children {
        assert!(child.wait()?.success());
    }
    let mut outcomes = ["a", "b"]
        .into_iter()
        .map(|child_id| {
            std::fs::read_to_string(temp.path().join(format!("result-{child_id}")))
                .map_err(ArtifactError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    outcomes.sort();
    assert_eq!(
        outcomes,
        vec!["committed".to_string(), "quota_exceeded".to_string()]
    );
    Ok(())
}
