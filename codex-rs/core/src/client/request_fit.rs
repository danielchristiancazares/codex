//! Final logical-request accounting, including formatted envelopes and tool schemas.

use codex_api::ResponsesApiRequest;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result;
use codex_protocol::openai_models::ModelInfo;
use codex_utils_output_truncation::approx_token_count;

pub(super) fn check(request: &ResponsesApiRequest, model: &ModelInfo) -> Result<()> {
    let input_tokens = request
        .input
        .iter()
        .map(crate::context_manager::estimate_item_token_count)
        .fold(0i64, i64::saturating_add);
    // Provider normalization can remove fields, so this includes every potentially
    // visible schema and text-format envelope before either streaming transport sends.
    let schema_tokens =
        approx_token_count(&serde_json::to_string(&(&request.tools, &request.text))?);
    let estimate = input_tokens
        .saturating_add(
            i64::try_from(approx_token_count(&request.instructions)).unwrap_or(i64::MAX),
        )
        .saturating_add(i64::try_from(schema_tokens).unwrap_or(i64::MAX));
    crate::context_manager::RequestBudget::for_model(model)
        .check(estimate)
        .map_err(|_| {
            codex_protocol::error::CodexErr::from(CodexErrorDetails::ContextWindowExceeded)
        })?;
    Ok(())
}
