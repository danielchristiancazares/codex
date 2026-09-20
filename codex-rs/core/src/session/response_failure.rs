//! Usage accounting for provider-declared terminal response failures.

use super::session::Session;
use super::step_settings::ResolvedStepSettings;
use super::turn_context::TurnContext;
use codex_history::RolloutItem;
use codex_protocol::IncompleteResponse;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TokenUsage;

impl Session {
    pub(crate) async fn record_response_token_usage(
        &self,
        turn_context: &TurnContext,
        response_id: &str,
        usage: &TokenUsage,
    ) {
        let record = self.state.lock().await.record_token_usage(
            self.thread_id,
            &turn_context.sub_id,
            self.session_id(),
            turn_context
                .turn_metadata_state
                .root_turn_id()
                .unwrap_or_else(|| turn_context.sub_id.clone()),
            response_id.to_string(),
            usage,
        );
        self.persist_rollout_items(&[RolloutItem::TokenUsageRecord(record)])
            .await;
    }

    pub(crate) async fn record_incomplete_response_usage(
        &self,
        turn_context: &TurnContext,
        settings: &ResolvedStepSettings,
        failure: &IncompleteResponse,
    ) -> CodexResult<()> {
        let Some(usage) = failure.token_usage.as_ref() else {
            // Missing provider usage does not establish a new billed total.
            return Ok(());
        };
        if let Some(response_id) = failure.response_id.as_deref() {
            self.record_response_token_usage(turn_context, response_id, usage)
                .await;
        }
        self.record_token_usage_info(turn_context, settings, Some(usage))
            .await
    }
}
