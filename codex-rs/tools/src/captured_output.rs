//! Adapts captured text to recoverable tool previews without changing hook payloads.

use crate::ToolOutput;
use crate::ToolPayload;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_utils_output_truncation::CaptureReceipt;
use serde_json::Value;

/// Adds a capture receipt to model-visible output while retaining original hook payloads.
pub struct CapturedOutput<T> {
    original: T,
    receipt: CaptureReceipt,
}

impl<T> CapturedOutput<T> {
    pub fn new(original: T, receipt: CaptureReceipt) -> Self {
        Self { original, receipt }
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

    fn tool_result_metadata(&self) -> Option<&Value> {
        self.original.tool_result_metadata()
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
