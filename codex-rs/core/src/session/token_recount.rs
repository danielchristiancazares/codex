//! Recomputes filtered history usage without publishing events before startup.
use super::*;

pub(super) enum TokenUsageDelivery {
    SeedSession,
    NotifyClients,
}

impl Session {
    pub(super) async fn recount_token_usage(
        &self,
        turn_context: &TurnContext,
        delivery: TokenUsageDelivery,
    ) {
        let history = self.clone_history().await;
        let base_instructions = self.get_base_instructions().await;
        let Some(estimated_total_tokens) =
            history.estimate_token_count_with_base_instructions(&base_instructions)
        else {
            return;
        };
        let estimated_total_tokens = estimated_total_tokens
            .saturating_add(
                history.model_visible_token_delta(&turn_context.model_info().input_modalities),
            )
            .max(0);
        {
            let mut state = self.state.lock().await;
            let mut info = state.token_info().unwrap_or(TokenUsageInfo {
                total_token_usage: TokenUsage::default(),
                last_token_usage: TokenUsage::default(),
                model_context_window: None,
            });

            info.last_token_usage = TokenUsage {
                input_tokens: 0,
                cached_input_tokens: 0,
                cache_write_input_tokens: 0,
                output_tokens: 0,
                reasoning_output_tokens: 0,
                total_tokens: estimated_total_tokens.max(0),
                codex_rollout_budget_units: None,
            };

            if let Some(model_context_window) = turn_context.model_context_window() {
                info.model_context_window = Some(model_context_window);
            }

            state.set_token_info(Some(info));
        }
        self.set_auto_compact_window_estimated_prefill_for_scope(
            turn_context,
            estimated_total_tokens,
        )
        .await;
        match delivery {
            TokenUsageDelivery::SeedSession => {}
            TokenUsageDelivery::NotifyClients => self.send_token_count_event(turn_context).await,
        }
    }
}
