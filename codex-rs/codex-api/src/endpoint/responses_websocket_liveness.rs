use super::ApiError;
use super::Message;
use super::ResponsesStreamEvent;
use super::WsError;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio::time::sleep_until;

/// Tracks transport silence separately from bounded application progress.
pub(super) struct ResponseLiveness {
    idle_timeout: Duration,
    request_started: Instant,
    progress_deadline: Instant,
}

impl ResponseLiveness {
    pub(super) fn new(idle_timeout: Duration) -> Self {
        let request_started = Instant::now();
        Self {
            idle_timeout,
            request_started,
            progress_deadline: request_started + idle_timeout.saturating_mul(4),
        }
    }

    pub(super) fn record_event(&mut self, event: &ResponsesStreamEvent) {
        // Metadata and heartbeats prove transport activity only. Acknowledgement
        // and generated output allow a bounded, longer reasoning interval.
        match event.kind() {
            "response.created"
            | "response.in_progress"
            | "response.output_item.added"
            | "response.output_item.done"
            | "response.content_part.added"
            | "response.content_part.done"
            | "response.output_text.delta"
            | "response.output_text.done"
            | "response.reasoning_text.delta"
            | "response.reasoning_text.done"
            | "response.reasoning_summary_text.delta"
            | "response.reasoning_summary_text.done"
            | "response.function_call_arguments.delta"
            | "response.function_call_arguments.done"
            | "response.custom_tool_call_input.delta"
            | "response.custom_tool_call_input.done" => {
                self.progress_deadline = Instant::now() + self.idle_timeout.saturating_mul(4);
            }
            _ => {}
        }
    }

    pub(super) async fn next_message(
        &self,
        messages: &mut mpsc::UnboundedReceiver<Result<Message, WsError>>,
        activity: &mut watch::Receiver<Instant>,
    ) -> Result<Message, ApiError> {
        loop {
            let transport_deadline =
                (*activity.borrow_and_update()).max(self.request_started) + self.idle_timeout;
            tokio::select! {
                biased;
                _ = sleep_until(self.progress_deadline) => {
                    return Err(ApiError::Stream("response progress timeout waiting for websocket".into()));
                }
                message = messages.recv() => return match message {
                    Some(Ok(message)) => Ok(message),
                    Some(Err(error)) => Err(ApiError::Stream(error.to_string())),
                    None => Err(ApiError::Stream("stream closed before response.completed".into())),
                },
                changed = activity.changed() => {
                    if changed.is_err() {
                        return Err(ApiError::Stream("websocket transport closed".into()));
                    }
                }
                _ = sleep_until(transport_deadline) => {
                    return Err(ApiError::Stream("idle timeout waiting for websocket".into()));
                }
            }
        }
    }
}
