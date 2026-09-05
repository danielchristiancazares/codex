//! Size projection for both completed history and snapshots taken during tool execution.

use super::ContextManager;
use super::estimate_item_token_count;
use crate::context_manager::citation_projection;
use crate::context_manager::normalize;
use codex_protocol::openai_models::InputModality;
use codex_utils_output_truncation::TruncationPolicy;
use std::sync::Arc;

impl ContextManager {
    pub(crate) fn model_visible_token_delta(
        &self,
        input_modalities: &[InputModality],
        truncation_policy: TruncationPolicy,
    ) -> i64 {
        let raw_estimate = self
            .items
            .iter()
            .map(|envelope| estimate_item_token_count(&envelope.item))
            .fold(0i64, i64::saturating_add);
        let mut history = self.clone();
        // A live call can query its own remaining budget before its output exists.
        // Preserve pending exchanges; final prompt preparation still enforces pairing.
        history.project_model_visible_content(input_modalities);
        history.finalize_function_outputs(truncation_policy);
        let model_visible_estimate = history
            .items
            .iter()
            .map(|envelope| estimate_item_token_count(&envelope.item))
            .fold(0i64, i64::saturating_add);
        model_visible_estimate.saturating_sub(raw_estimate)
    }

    pub(super) fn project_model_visible_content(&mut self, input_modalities: &[InputModality]) {
        let items = Arc::make_mut(&mut self.items);
        citation_projection::strip_hidden_citations(items);
        normalize::strip_images_when_unsupported(input_modalities, items);
        normalize::strip_audio_when_unsupported(input_modalities, items);
    }
}

#[cfg(test)]
#[path = "history_token_projection_tests.rs"]
mod tests;
