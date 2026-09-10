//! Diagnostic request estimates; provider tokenization determines acceptance.

use crate::responses_metadata::CodexResponsesMetadata;
use codex_api::ResponsesApiRequest;
use codex_protocol::openai_models::ModelInfo;
use codex_utils_output_truncation::approx_token_count;

pub(super) fn record(
    request: &ResponsesApiRequest,
    model: &ModelInfo,
    metadata: &CodexResponsesMetadata,
) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        return;
    }
    let Some(usable_context_window) = model.usable_context_window() else {
        return;
    };
    // Diagnostic serialization must not prevent the provider from receiving a request.
    let Ok(schemas) = serde_json::to_string(&(&request.tools, &request.text)) else {
        return;
    };
    let input_tokens = request
        .input
        .iter()
        .map(crate::context_manager::estimate_item_token_count)
        .fold(0i64, i64::saturating_add);
    // Provider normalization can remove fields, so this includes every potentially
    // visible schema and text-format envelope before either streaming transport sends.
    let schema_tokens = i64::try_from(approx_token_count(&schemas)).unwrap_or(i64::MAX);
    let instructions_tokens =
        i64::try_from(approx_token_count(&request.instructions)).unwrap_or(i64::MAX);
    let estimated_tokens = input_tokens
        .saturating_add(instructions_tokens)
        .saturating_add(schema_tokens);
    if crate::context_manager::RequestBudget::for_model(model)
        .check(estimated_tokens)
        .is_err()
    {
        tracing::debug!(
            model = %model.slug,
            thread_id = %metadata.thread_id,
            turn_id = ?metadata.turn_id,
            request_kind = ?metadata.request_kind,
            input_tokens,
            instructions_tokens,
            schema_tokens,
            estimated_tokens,
            usable_context_window,
            "Model request estimate exceeds usable context window"
        );
    }
}
