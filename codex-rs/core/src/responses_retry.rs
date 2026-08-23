//! Shared retry decisions for Responses requests.

use std::time::Duration;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::util::backoff;
use codex_client::RetryOperation;
use codex_features::Feature;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use tracing::warn;

const INITIAL_CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(5);
const MAX_UNBOUNDED_RETRY_DELAY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResponsesStreamRequest {
    Sampling,
    RemoteCompactionV2,
}

pub(crate) struct ResponsesStreamRetryState {
    retries: u64,
    connection_retries: u64,
    connection_retry_delay: Duration,
}

impl Default for ResponsesStreamRetryState {
    fn default() -> Self {
        Self {
            retries: 0,
            connection_retries: 0,
            connection_retry_delay: INITIAL_CONNECTION_RETRY_DELAY,
        }
    }
}

/// Handles a retryable stream error and returns `Ok(())` when the caller should
/// retry the request loop.
pub(crate) async fn handle_retryable_response_stream_error(
    retry_state: &mut ResponsesStreamRetryState,
    max_retries: u64,
    err: CodexErr,
    sess: &Session,
    turn_context: &TurnContext,
    request: ResponsesStreamRequest,
) -> Result<(), CodexErr> {
    let operation = match request {
        ResponsesStreamRequest::Sampling => RetryOperation::Sampling,
        ResponsesStreamRequest::RemoteCompactionV2 => RetryOperation::RemoteCompactionV2,
    };

    if turn_context
        .config
        .features
        .enabled(Feature::UnboundedConnectionRetries)
        && matches!(request, ResponsesStreamRequest::Sampling)
        && matches!(err.details(), CodexErrorDetails::ConnectionFailed(_))
        && !turn_context.session_source.is_internal()
    {
        let retry_delay = retry_state.connection_retry_delay;
        warn!(
            turn_id = %turn_context.sub_id,
            error = %err,
            ?retry_delay,
            "stream connection failed; waiting to retry"
        );
        sess.notify_stream_error(turn_context, "Reconnecting... waiting for network", err)
            .await;
        retry_state.connection_retries = retry_state.connection_retries.saturating_add(1);
        codex_client::record_retry!(retry_state.connection_retries, retry_delay, operation);
        tokio::time::sleep(retry_delay).await;
        retry_state.connection_retry_delay =
            retry_delay.saturating_mul(2).min(MAX_UNBOUNDED_RETRY_DELAY);
        return Ok(());
    }

    let websocket_retries_unbounded = sess.services.model_client.responses_websocket_enabled();
    if websocket_retries_unbounded || retry_state.retries < max_retries {
        retry_state.retries = retry_state.retries.saturating_add(1);
        let retry_count = retry_state.retries;
        let delay = err.retry_delay().unwrap_or_else(|| {
            let delay = backoff(retry_count);
            if websocket_retries_unbounded {
                delay.min(MAX_UNBOUNDED_RETRY_DELAY)
            } else {
                delay
            }
        });
        if websocket_retries_unbounded {
            match request {
                ResponsesStreamRequest::Sampling => {
                    warn!(
                        turn_id = %turn_context.sub_id,
                        retries = retry_count,
                        sampling_error = %err,
                        "websocket stream disconnected - retrying sampling request in {delay:?}"
                    );
                }
                ResponsesStreamRequest::RemoteCompactionV2 => {
                    warn!(
                        turn_id = %turn_context.sub_id,
                        retries = retry_count,
                        compact_error = %err,
                        "websocket remote compaction stream failed; retrying request in {delay:?}"
                    );
                }
            }
        } else {
            log_retry(request, turn_context, &err, retry_count, max_retries, delay);
        }

        // In release builds, hide the first websocket retry notification to reduce noisy
        // transient reconnect messages. In debug builds, keep full visibility for diagnosis.
        let report_error =
            retry_count > 1 || cfg!(debug_assertions) || !websocket_retries_unbounded;
        if report_error {
            // Surface retry information to any UI/front-end so the user understands what is
            // happening instead of staring at a seemingly frozen screen.
            let message = if websocket_retries_unbounded {
                format!("Reconnecting... {retry_count}")
            } else {
                format!("Reconnecting... {retry_count}/{max_retries}")
            };
            sess.notify_stream_error(turn_context, message, err).await;
        }
        codex_client::record_retry!(retry_count, delay, operation);
        tokio::time::sleep(delay).await;
        return Ok(());
    }

    Err(err)
}

fn log_retry(
    request: ResponsesStreamRequest,
    turn_context: &TurnContext,
    err: &CodexErr,
    retries: u64,
    max_retries: u64,
    delay: Duration,
) {
    match request {
        ResponsesStreamRequest::Sampling => {
            warn!(
                turn_id = %turn_context.sub_id,
                retries,
                max_retries,
                sampling_error = %err,
                "stream disconnected - retrying sampling request ({retries}/{max_retries} in {delay:?})...",
            );
        }
        ResponsesStreamRequest::RemoteCompactionV2 => {
            warn!(
                turn_id = %turn_context.sub_id,
                retries,
                max_retries,
                compact_error = %err,
                "remote compaction v2 stream failed; retrying request after delay"
            );
        }
    }
}

#[cfg(test)]
#[path = "responses_retry_tests.rs"]
mod tests;
