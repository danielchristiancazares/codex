use codex_exec_output_artifacts::ArtifactCapture;
use codex_exec_output_artifacts::ArtifactCaptureSnapshot;
use codex_exec_output_artifacts::ArtifactCaptureStatus;
use codex_exec_output_artifacts::ArtifactProducer;
use codex_exec_output_artifacts::ArtifactProducerKind;
use codex_exec_output_artifacts::ArtifactQuery;
use codex_exec_output_artifacts::ArtifactQueryData;
use codex_exec_output_artifacts::ArtifactQueryPresentation;
use codex_exec_output_artifacts::ArtifactStore;
use codex_exec_output_artifacts::ArtifactStoreConfig;
use codex_exec_output_artifacts::ArtifactStream;
use codex_exec_output_artifacts::preview_sha256 as exec_output_preview_digest;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::ExecOutputArtifactCapture;
use super::Finalization;
use super::RawExecOutputCapture;
use super::exec_output_artifact_access;
use super::looks_binary;
use super::sanitize_process_stream;
use crate::session::tests::make_session_and_context;
use crate::unified_exec::head_tail_buffer::HeadTailBuffer;
use crate::unified_exec::process::OutputHandles;

#[test]
fn sanitization_happens_before_artifact_persistence() {
    let sanitized = sanitize_process_stream(
        ArtifactCaptureSnapshot {
            bytes: b"token=abcdefghijklmnopqrstuvwxyz\nsafe".to_vec(),
            observed_byte_count: 37,
            capture: ArtifactCapture::Complete,
        },
        /*durable_bytes_cap*/ usize::MAX,
    );

    assert_eq!(
        String::from_utf8(sanitized.bytes).expect("sanitized artifact is UTF-8"),
        "token=[REDACTED_SECRET]\nsafe"
    );
    assert_eq!(sanitized.media_type, "text/plain");
}

#[test]
fn binary_streams_become_non_content_summaries() {
    let sanitized = sanitize_process_stream(
        ArtifactCaptureSnapshot {
            bytes: b"secret\0bytes".to_vec(),
            observed_byte_count: 12,
            capture: ArtifactCapture::Complete,
        },
        /*durable_bytes_cap*/ usize::MAX,
    );
    let rendered = String::from_utf8(sanitized.bytes).expect("binary summary is UTF-8");

    assert!(looks_binary(b"secret\0bytes"));
    assert!(!rendered.contains("secret"));
    assert!(rendered.contains("\"contentExposed\":false"));
    assert_eq!(
        sanitized.media_type,
        "application/vnd.codex.binary-summary+json"
    );
}

#[test]
fn expanded_encoding_is_persisted_as_a_bounded_utf8_prefix() -> anyhow::Result<()> {
    const DURABLE_BYTES_CAP: usize = 22;
    let raw_bytes = b"\x93\x94 test \x96 dash \x97".to_vec();
    assert!(raw_bytes.len() < DURABLE_BYTES_CAP);

    let temp = tempfile::tempdir()?;
    let store = ArtifactStore::open(
        temp.path(),
        "thread-a",
        ArtifactStoreConfig {
            artifact_bytes_cap: DURABLE_BYTES_CAP,
            ..ArtifactStoreConfig::default()
        },
    )?;
    let access = exec_output_artifact_access("thread-a", "local", ["file:///workspace"]);
    let reservation = store.reserve(
        access.clone(),
        ArtifactProducer {
            kind: ArtifactProducerKind::ProcessStream,
            item_id: "call-1".to_string(),
            process_id: Some("1000".to_string()),
            stream: ArtifactStream::Stdout,
        },
    )?;
    let descriptor = store.complete(
        &reservation,
        sanitize_process_stream(
            ArtifactCaptureSnapshot {
                observed_byte_count: u64::try_from(raw_bytes.len())?,
                bytes: raw_bytes,
                capture: ArtifactCapture::Complete,
            },
            DURABLE_BYTES_CAP,
        ),
    )?;

    assert_eq!(
        (
            descriptor.byte_count,
            descriptor.observed_byte_count,
            descriptor.capture,
        ),
        (21, 16, ArtifactCapture::Truncated)
    );
    let query = store.query(
        &descriptor.artifact_ref,
        &access,
        &ArtifactQuery::Head {
            max_bytes: DURABLE_BYTES_CAP,
        },
        &ArtifactQueryPresentation::include_data("turn-1:0"),
    )?;
    assert_eq!(
        query.data,
        Some(ArtifactQueryData::Text {
            text: "\u{201c}\u{201d} test \u{2013} dash ".to_string(),
            byte_start: 0,
            byte_end: 21,
            line_start: Some(1),
            line_end: Some(1),
            truncated: false,
        })
    );
    Ok(())
}

#[test]
fn preview_digest_is_stable() {
    assert_eq!(
        exec_output_preview_digest("bounded preview"),
        exec_output_preview_digest("bounded preview")
    );
}

#[test]
fn lost_output_seals_capture_at_the_contiguous_prefix() {
    let capture = RawExecOutputCapture::new(/*byte_cap*/ 1024);
    capture.record(ArtifactStream::Stdout, b"prefix");
    capture.mark_truncated();
    capture.record(ArtifactStream::Stdout, b"suffix");

    assert_eq!(
        capture.status(ArtifactStream::Stdout),
        ArtifactCaptureStatus {
            retained_byte_count: 6,
            observed_byte_count: 6,
            capture: ArtifactCapture::Truncated,
        }
    );
}

#[tokio::test]
async fn finalization_continues_when_the_first_waiter_is_cancelled() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let store = ArtifactStore::open(temp.path(), "thread-a", ArtifactStoreConfig::default())?;
    let raw_capture = RawExecOutputCapture::new(/*byte_cap*/ 1024);
    raw_capture.record(ArtifactStream::Stdout, b"complete me");
    let output_closed = Arc::new(AtomicBool::new(false));
    let output_closed_notify = Arc::new(Notify::new());
    let output = OutputHandles {
        output_buffer: Arc::new(tokio::sync::Mutex::new(HeadTailBuffer::default())),
        artifact_capture: Some(Arc::clone(&raw_capture)),
        output_notify: Arc::new(Notify::new()),
        output_closed: Arc::clone(&output_closed),
        output_closed_notify: Arc::clone(&output_closed_notify),
        cancellation_token: CancellationToken::new(),
    };
    let (session, _) = make_session_and_context().await;
    let capture = ExecOutputArtifactCapture::reserve(
        store,
        exec_output_artifact_access("thread-a", "local", ["file:///workspace"]),
        "call-1",
        /*process_id*/ 1000,
        &output,
        session.services.session_telemetry.clone(),
    )?;

    let first_capture = Arc::clone(&capture);
    let first_waiter = tokio::spawn(async move { first_capture.finalize().await });
    loop {
        let running = matches!(
            *capture
                .finalization
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            Finalization::Running
        );
        if running {
            break;
        }
        tokio::task::yield_now().await;
    }
    first_waiter.abort();
    output_closed.store(true, Ordering::Release);
    output_closed_notify.notify_waiters();

    let artifacts = capture.finalize().await?;
    assert_eq!(
        artifacts.stdout.state,
        codex_exec_output_artifacts::ArtifactState::Complete
    );
    assert_eq!(artifacts.stdout.byte_count, 11);
    Ok(())
}
