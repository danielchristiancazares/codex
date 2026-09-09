//! Adapts captured text to recoverable tool previews without changing hook payloads.

mod read_output;
pub(crate) use read_output::ReadOutputHandler;

use super::context::ExecCommandToolOutput;
use super::context::ToolOutput;
use super::context::ToolPayload;
use crate::session::session::Session;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ToolResultSources;
use codex_utils_output_truncation::CaptureId;
use codex_utils_output_truncation::CaptureReceipt;
use serde_json::Value;

pub(crate) struct CapturedOutput<T> {
    original: T,
    receipt: CaptureReceipt,
}

impl<T> CapturedOutput<T> {
    pub(crate) async fn new(
        session: &Session,
        original: T,
        text: String,
        omitted_bytes: usize,
    ) -> Self {
        let receipt = session.services.captured_output.lock().await.insert(
            CaptureId::new(uuid::Uuid::new_v4().into_bytes()),
            text,
            omitted_bytes,
        );
        Self { original, receipt }
    }
}

impl CapturedOutput<ExecCommandToolOutput> {
    pub(crate) async fn terminal(session: &Session, original: ExecCommandToolOutput) -> Self {
        let text = String::from_utf8_lossy(&original.raw_output).into_owned();
        let omitted_bytes = original
            .output_omitted_bytes
            .map_or(0, std::num::NonZeroUsize::get);
        Self::new(session, original, text, omitted_bytes).await
    }
}

impl<T: ToolOutput> ToolOutput for CapturedOutput<T> {
    fn log_output(&self) -> String {
        self.original.log_output()
    }

    fn success_for_logging(&self) -> bool {
        self.original.success_for_logging()
    }

    fn contains_external_context(&self) -> bool {
        self.original.contains_external_context()
    }

    fn fallback_token_limit_override(&self) -> Option<usize> {
        // These adapters already enforce the requested/effective preview limit.
        // Preserve the separately bounded receipt through history retention.
        Some(10_000)
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        let mut response = self.original.to_response_item(call_id, payload);
        match &mut response {
            ResponseInputItem::FunctionCallOutput { output, .. }
            | ResponseInputItem::CustomToolCallOutput { output, .. } => match &mut output.body {
                FunctionCallOutputBody::Text(text) => {
                    text.insert_str(0, &self.receipt.notice());
                }
                FunctionCallOutputBody::ContentItems(items) => match items.first_mut() {
                    Some(FunctionCallOutputContentItem::InputText { text }) => {
                        text.insert_str(0, &self.receipt.notice())
                    }
                    _ => items.insert(
                        0,
                        FunctionCallOutputContentItem::InputText {
                            text: self.receipt.notice().to_string(),
                        },
                    ),
                },
            },
            ResponseInputItem::Message { .. }
            | ResponseInputItem::McpToolCallOutput { .. }
            | ResponseInputItem::ToolSearchOutput { .. } => {}
        }
        response
    }

    fn post_tool_use_id(&self, call_id: &str) -> String {
        self.original.post_tool_use_id(call_id)
    }

    fn post_tool_use_input(&self, payload: &ToolPayload) -> Option<Value> {
        self.original.post_tool_use_input(payload)
    }

    fn post_tool_use_response(&self, call_id: &str, payload: &ToolPayload) -> Option<Value> {
        self.original.post_tool_use_response(call_id, payload)
    }

    fn tool_result_sources(&self) -> Option<ToolResultSources> {
        self.original.tool_result_sources()
    }

    fn code_mode_result(&self, payload: &ToolPayload) -> Value {
        let mut result = self.original.code_mode_result(payload);
        match &mut result {
            Value::Object(fields) => {
                fields.insert(
                    "output_id".to_string(),
                    Value::String(self.receipt.id().to_string()),
                );
                fields.insert(
                    "output_capture".to_string(),
                    Value::String(self.receipt.notice().to_string()),
                );
            }
            Value::String(text) => text.insert_str(0, &self.receipt.notice()),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => {}
        }
        result
    }
}

#[cfg(test)]
#[path = "captured_output_tests.rs"]
mod tests;
