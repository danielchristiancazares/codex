//! Reserves the serialized tool-result envelope within the context-item ceiling.

use super::function_output::MAX_FUNCTION_OUTPUT_TOKENS;
use crate::utils::json::serialized_json_bytes;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;

/// Returns the output allowance after conservatively charging one token per
/// envelope byte. The body is excluded without cloning its text or media.
/// An envelope that exhausts the ceiling receives zero allowance and is rejected
/// by the final outbound check if even the empty output cannot fit.
pub(crate) fn function_output_item_token_budget(item: &ResponseItem) -> Option<usize> {
    let frame = match item {
        ResponseItem::FunctionCallOutput {
            id,
            call_id,
            name,
            namespace,
            internal_chat_message_metadata_passthrough,
            ..
        } => ResponseItem::FunctionCallOutput {
            id: id.clone(),
            call_id: call_id.clone(),
            name: name.clone(),
            namespace: namespace.clone(),
            output: FunctionCallOutputPayload::default(),
            internal_chat_message_metadata_passthrough: internal_chat_message_metadata_passthrough
                .clone(),
        },
        ResponseItem::CustomToolCallOutput {
            id,
            call_id,
            name,
            internal_chat_message_metadata_passthrough,
            ..
        } => ResponseItem::CustomToolCallOutput {
            id: id.clone(),
            call_id: call_id.clone(),
            name: name.clone(),
            output: FunctionCallOutputPayload::default(),
            internal_chat_message_metadata_passthrough: internal_chat_message_metadata_passthrough
                .clone(),
        },
        ResponseItem::ConfigurationUpdate { .. }
        | ResponseItem::AdditionalTools { .. }
        | ResponseItem::Message { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => return None,
    };
    // The default output serializes as an empty JSON string (two quote bytes).
    let envelope_bytes = serialized_json_bytes(&frame)
        .unwrap_or(usize::MAX)
        .saturating_sub(2);
    Some(MAX_FUNCTION_OUTPUT_TOKENS.saturating_sub(envelope_bytes))
}
