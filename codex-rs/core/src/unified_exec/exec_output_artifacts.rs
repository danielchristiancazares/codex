use codex_exec_output_artifacts::ArtifactAccess;
use codex_exec_output_artifacts::ArtifactCapture;
use codex_exec_output_artifacts::ArtifactCaptureBuffer;
use codex_exec_output_artifacts::ArtifactDescriptor;
use codex_exec_output_artifacts::ArtifactError;
use codex_exec_output_artifacts::ArtifactProducer;
use codex_exec_output_artifacts::ArtifactProducerKind;
use codex_exec_output_artifacts::ArtifactReservation;
use codex_exec_output_artifacts::ArtifactStore;
use codex_exec_output_artifacts::ArtifactStream;
use codex_exec_output_artifacts::ExecOutputArtifacts;
use codex_exec_output_artifacts::SanitizedArtifactContent;
use codex_otel::EXEC_OUTPUT_ARTIFACT_FULL_OUTPUT_BYTES_METRIC;
use codex_otel::EXEC_OUTPUT_ARTIFACT_PRESENTED_OUTPUT_BYTES_METRIC;
use codex_otel::EXEC_OUTPUT_ARTIFACT_PREVIEW_TRUNCATION_METRIC;
use codex_otel::SessionTelemetry;
use codex_protocol::exec_output::bytes_to_string_smart;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use super::process::OutputHandles;
use crate::tools::context::ExecCommandToolOutput;
use crate::unified_exec::ExecCommandRequest;
use crate::unified_exec::UnifiedExecContext;

pub(crate) const EXEC_OUTPUT_ARTIFACT_PREVIEW_MAX_TOKENS: usize = 512;
const OUTPUT_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) fn reserve_exec_output_artifacts(
    context: &UnifiedExecContext,
    request: &ExecCommandRequest,
    output: &OutputHandles,
) -> Option<Arc<ExecOutputArtifactCapture>> {
    let store = context
        .session
        .services
        .thread_extension_data
        .get::<ArtifactStore>()?;
    let access = exec_output_artifact_access(
        context.session.thread_id,
        &request.turn_environment.selection.environment_id,
        request.turn_environment.selection.workspace_roots.iter(),
    );
    match ExecOutputArtifactCapture::reserve(
        store.as_ref().clone(),
        access,
        &context.call_id,
        request.process_id,
        output,
        context.session.services.session_telemetry.clone(),
    ) {
        Ok(capture) => Some(capture),
        Err(err) => {
            tracing::warn!(error = %err, "failed to reserve exec-output artifacts");
            None
        }
    }
}

pub(crate) async fn finalize_exec_output_artifacts(
    capture: Option<&Arc<ExecOutputArtifactCapture>>,
) -> Option<ExecOutputArtifacts> {
    match capture {
        Some(capture) => capture.finalize().await.ok(),
        None => None,
    }
}

pub(crate) fn exec_output_for_model(
    artifacts: &Option<ExecOutputArtifacts>,
    collected: Vec<u8>,
) -> Vec<u8> {
    if artifacts.is_some() {
        sanitize_exec_output_for_model(collected)
    } else {
        collected
    }
}

pub(crate) fn record_exec_output_artifact_preview_metrics(
    session_telemetry: &SessionTelemetry,
    output: &ExecCommandToolOutput,
) {
    let Some((presented_bytes, truncated)) = output.artifact_preview_stats() else {
        return;
    };
    session_telemetry.histogram(
        EXEC_OUTPUT_ARTIFACT_PRESENTED_OUTPUT_BYTES_METRIC,
        i64::try_from(presented_bytes).unwrap_or(i64::MAX),
        &[("source", "exec")],
    );
    if truncated {
        session_telemetry.counter(
            EXEC_OUTPUT_ARTIFACT_PREVIEW_TRUNCATION_METRIC,
            /*inc*/ 1,
            &[("source", "exec")],
        );
    }
}

pub(crate) struct ExecOutputArtifactCapture {
    store: ArtifactStore,
    raw_capture: Arc<RawExecOutputCapture>,
    stdout: ReservedStream,
    stderr: ReservedStream,
    output_closed: Arc<AtomicBool>,
    output_closed_notify: Arc<Notify>,
    session_telemetry: SessionTelemetry,
    finalization: Mutex<Finalization>,
    finalization_notify: Notify,
}

pub(crate) struct RawExecOutputCapture {
    stdout: Arc<ArtifactCaptureBuffer>,
    stderr: Arc<ArtifactCaptureBuffer>,
    recording: AtomicBool,
}

struct CaptureTaskGuard {
    capture: Option<Arc<RawExecOutputCapture>>,
    complete: bool,
}

struct ReservedStream {
    reservation: ArtifactReservation,
}

enum Finalization {
    Pending,
    Running,
    Complete(Box<ExecOutputArtifacts>),
    Failed,
}

impl RawExecOutputCapture {
    pub(crate) fn new(byte_cap: usize) -> Arc<Self> {
        Arc::new(Self {
            stdout: Arc::new(ArtifactCaptureBuffer::new(byte_cap)),
            stderr: Arc::new(ArtifactCaptureBuffer::new(byte_cap)),
            recording: AtomicBool::new(true),
        })
    }

    pub(crate) fn record(&self, stream: ArtifactStream, bytes: &[u8]) {
        if !self.recording.load(Ordering::Acquire) {
            return;
        }
        match stream {
            ArtifactStream::Stdout => self.stdout.push(bytes),
            ArtifactStream::Stderr => self.stderr.push(bytes),
        }
    }

    pub(crate) fn mark_truncated(&self) {
        self.recording.store(false, Ordering::Release);
        self.stdout.mark_truncated();
        self.stderr.mark_truncated();
    }

    pub(crate) fn status(
        &self,
        stream: ArtifactStream,
    ) -> codex_exec_output_artifacts::ArtifactCaptureStatus {
        match stream {
            ArtifactStream::Stdout => self.stdout.status(),
            ArtifactStream::Stderr => self.stderr.status(),
        }
    }

    fn take(&self, stream: ArtifactStream) -> codex_exec_output_artifacts::ArtifactCaptureSnapshot {
        match stream {
            ArtifactStream::Stdout => self.stdout.take(),
            ArtifactStream::Stderr => self.stderr.take(),
        }
    }
}

impl Drop for CaptureTaskGuard {
    fn drop(&mut self) {
        if !self.complete
            && let Some(capture) = self.capture.as_ref()
        {
            capture.mark_truncated();
        }
    }
}

pub(crate) fn combine_captured_output_receivers(
    mut stdout_rx: mpsc::Receiver<Vec<u8>>,
    mut stderr_rx: mpsc::Receiver<Vec<u8>>,
    capture: Option<Arc<RawExecOutputCapture>>,
    output_lost: Arc<AtomicBool>,
) -> broadcast::Receiver<Vec<u8>> {
    let (combined_tx, combined_rx) = broadcast::channel(/*capacity*/ 256);
    tokio::spawn(async move {
        let mut guard = CaptureTaskGuard {
            capture: capture.clone(),
            complete: false,
        };
        let mut stdout_open = true;
        let mut stderr_open = true;
        while stdout_open || stderr_open {
            tokio::select! {
                chunk = stdout_rx.recv(), if stdout_open => match chunk {
                    Some(chunk) => {
                        if let Some(capture) = capture.as_ref() {
                            capture.record(ArtifactStream::Stdout, &chunk);
                        }
                        let _ = combined_tx.send(chunk);
                    }
                    None => stdout_open = false,
                },
                chunk = stderr_rx.recv(), if stderr_open => match chunk {
                    Some(chunk) => {
                        if let Some(capture) = capture.as_ref() {
                            capture.record(ArtifactStream::Stderr, &chunk);
                        }
                        let _ = combined_tx.send(chunk);
                    }
                    None => stderr_open = false,
                },
            }
        }
        guard.complete = !output_lost.load(Ordering::Acquire);
    });
    combined_rx
}

impl ExecOutputArtifactCapture {
    pub(crate) fn reserve(
        store: ArtifactStore,
        access: ArtifactAccess,
        item_id: &str,
        process_id: i32,
        output: &OutputHandles,
        session_telemetry: SessionTelemetry,
    ) -> Result<Arc<Self>, ArtifactError> {
        let capture = output
            .artifact_capture
            .as_ref()
            .ok_or(ArtifactError::Incomplete)?
            .clone();
        let process_id = Some(process_id.to_string());
        let stdout = ReservedStream {
            reservation: store.reserve(
                access.clone(),
                ArtifactProducer {
                    kind: ArtifactProducerKind::ProcessStream,
                    item_id: item_id.to_string(),
                    process_id: process_id.clone(),
                    stream: ArtifactStream::Stdout,
                },
            )?,
        };
        let stderr = ReservedStream {
            reservation: store.reserve(
                access,
                ArtifactProducer {
                    kind: ArtifactProducerKind::ProcessStream,
                    item_id: item_id.to_string(),
                    process_id,
                    stream: ArtifactStream::Stderr,
                },
            )?,
        };
        Ok(Arc::new(Self {
            store,
            raw_capture: capture,
            stdout,
            stderr,
            output_closed: Arc::clone(&output.output_closed),
            output_closed_notify: Arc::clone(&output.output_closed_notify),
            session_telemetry,
            finalization: Mutex::new(Finalization::Pending),
            finalization_notify: Notify::new(),
        }))
    }

    pub(crate) fn pending_descriptors(&self) -> ExecOutputArtifacts {
        ExecOutputArtifacts {
            stdout: pending_descriptor(
                &self.stdout,
                self.raw_capture.status(ArtifactStream::Stdout),
            ),
            stderr: pending_descriptor(
                &self.stderr,
                self.raw_capture.status(ArtifactStream::Stderr),
            ),
        }
    }

    pub(crate) async fn finalize(self: &Arc<Self>) -> Result<ExecOutputArtifacts, ArtifactError> {
        loop {
            let notified = self.finalization_notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut state = self
                    .finalization
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &*state {
                    Finalization::Complete(artifacts) => return Ok((**artifacts).clone()),
                    Finalization::Failed => return Err(ArtifactError::FinalizationFailed),
                    Finalization::Pending => {
                        *state = Finalization::Running;
                        let capture = Arc::clone(self);
                        tokio::spawn(async move {
                            let result = capture.finalize_after_output().await;
                            let mut state = capture
                                .finalization
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            *state = match result {
                                Ok(artifacts) => Finalization::Complete(Box::new(artifacts)),
                                Err(err) => {
                                    tracing::warn!(
                                        error = %err,
                                        "failed to finalize exec-output artifacts"
                                    );
                                    Finalization::Failed
                                }
                            };
                            drop(state);
                            capture.finalization_notify.notify_waiters();
                        });
                    }
                    Finalization::Running => {}
                }
            }
            notified.await;
        }
    }

    async fn finalize_after_output(&self) -> Result<ExecOutputArtifacts, ArtifactError> {
        if !self.wait_for_output_close().await {
            self.raw_capture.mark_truncated();
        }
        self.finalize_inner().await
    }

    async fn wait_for_output_close(&self) -> bool {
        if self.output_closed.load(Ordering::Acquire) {
            return true;
        }
        let notified = self.output_closed_notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.output_closed.load(Ordering::Acquire) {
            return true;
        }
        tokio::time::timeout(OUTPUT_CLOSE_TIMEOUT, &mut notified)
            .await
            .is_ok()
            && self.output_closed.load(Ordering::Acquire)
    }

    async fn finalize_inner(&self) -> Result<ExecOutputArtifacts, ArtifactError> {
        let stdout = self.raw_capture.take(ArtifactStream::Stdout);
        let stderr = self.raw_capture.take(ArtifactStream::Stderr);
        let store = self.store.clone();
        let stdout_reservation = self.stdout.reservation.clone();
        let stderr_reservation = self.stderr.reservation.clone();
        tokio::task::spawn_blocking(move || {
            let artifact_bytes_cap = store.artifact_bytes_cap();
            let stdout = store.complete(
                &stdout_reservation,
                sanitize_process_stream(stdout, artifact_bytes_cap),
            )?;
            let stderr = store.complete(
                &stderr_reservation,
                sanitize_process_stream(stderr, artifact_bytes_cap),
            )?;
            Ok(ExecOutputArtifacts { stdout, stderr })
        })
        .await
        .map_err(|_| {
            ArtifactError::Storage(std::io::Error::other(
                "artifact finalization worker stopped unexpectedly",
            ))
        })?
        .inspect(|artifacts| {
            for (stream, descriptor) in
                [("stdout", &artifacts.stdout), ("stderr", &artifacts.stderr)]
            {
                self.session_telemetry.histogram(
                    EXEC_OUTPUT_ARTIFACT_FULL_OUTPUT_BYTES_METRIC,
                    i64::try_from(descriptor.observed_byte_count).unwrap_or(i64::MAX),
                    &[("stream", stream)],
                );
            }
        })
    }
}

fn pending_descriptor(
    stream: &ReservedStream,
    status: codex_exec_output_artifacts::ArtifactCaptureStatus,
) -> ArtifactDescriptor {
    stream.reservation.pending_descriptor(status)
}

fn sanitize_process_stream(
    snapshot: codex_exec_output_artifacts::ArtifactCaptureSnapshot,
    durable_bytes_cap: usize,
) -> SanitizedArtifactContent {
    let codex_exec_output_artifacts::ArtifactCaptureSnapshot {
        bytes,
        observed_byte_count,
        mut capture,
    } = snapshot;
    if looks_binary(&bytes) {
        let representation = serde_json::json!({
            "type": "binary_stream",
            "observedBytes": observed_byte_count,
            "capturedBytes": bytes.len(),
            "capture": match capture {
                ArtifactCapture::Complete => "complete",
                ArtifactCapture::Truncated => "truncated",
            },
            "contentExposed": false,
        })
        .to_string();
        return SanitizedArtifactContent {
            bytes: representation.into_bytes(),
            media_type: "application/vnd.codex.binary-summary+json".to_string(),
            observed_byte_count,
            capture,
        };
    }

    let decoded = bytes_to_string_smart(&bytes);
    let mut redacted = codex_secrets::redact_secrets(decoded);
    if redacted.len() > durable_bytes_cap {
        let mut boundary = durable_bytes_cap;
        while boundary > 0 && !redacted.is_char_boundary(boundary) {
            boundary -= 1;
        }
        redacted.truncate(boundary);
        capture = ArtifactCapture::Truncated;
    }
    SanitizedArtifactContent {
        bytes: redacted.into_bytes(),
        media_type: "text/plain".to_string(),
        observed_byte_count,
        capture,
    }
}

fn looks_binary(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return true;
    }
    let controls = bytes
        .iter()
        .filter(|byte| byte.is_ascii_control() && !matches!(byte, b'\n' | b'\r' | b'\t'))
        .count();
    controls.saturating_mul(20) > bytes.len()
}

pub(crate) fn exec_output_artifact_access(
    thread_id: impl ToString,
    environment_id: &str,
    workspace_roots: impl IntoIterator<Item = impl ToString>,
) -> ArtifactAccess {
    ArtifactAccess::new(
        thread_id.to_string(),
        environment_id,
        workspace_roots.into_iter().map(|root| root.to_string()),
    )
}

pub(crate) fn sanitize_exec_output_for_model(bytes: Vec<u8>) -> Vec<u8> {
    sanitize_process_stream(
        codex_exec_output_artifacts::ArtifactCaptureSnapshot {
            observed_byte_count: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            bytes,
            capture: ArtifactCapture::Complete,
        },
        /*durable_bytes_cap*/ usize::MAX,
    )
    .bytes
}

#[cfg(test)]
#[path = "exec_output_artifacts_tests.rs"]
mod tests;
