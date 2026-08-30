use std::sync::Arc;

use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutBudgetCheckpoint;
use codex_protocol::protocol::TokenCountEvent;

use super::AgentControl;

impl AgentControl {
    pub(crate) async fn persist_rollout_budget_checkpoint_for_other_threads(
        &self,
        current_thread_id: ThreadId,
        checkpoint: RolloutBudgetCheckpoint,
    ) {
        let Ok(manager) = self.upgrade() else {
            return;
        };
        for thread_id in manager.list_thread_ids().await {
            if thread_id == current_thread_id {
                continue;
            }
            let Ok(thread) = manager.get_thread(thread_id).await else {
                continue;
            };
            if !Arc::ptr_eq(
                &thread.session.services.agent_control.rollout_budget,
                &self.rollout_budget,
            ) {
                continue;
            }
            thread
                .session
                .persist_rollout_items(&[RolloutItem::EventMsg(EventMsg::TokenCount(
                    TokenCountEvent {
                        info: None,
                        rate_limits: None,
                        rollout_budget: Some(checkpoint),
                    },
                ))])
                .await;
        }
    }
}
