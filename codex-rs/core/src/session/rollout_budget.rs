use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_history::RolloutItem;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsage;

pub(super) async fn maybe_record_reminder(
    sess: &Session,
    turn_context: &TurnContext,
    window_id: &str,
) {
    let budget = sess.services.agent_control.rollout_budget();
    let Some(reminder) = budget.pending_reminder(sess.thread_id(), window_id) else {
        return;
    };
    let response_item = ContextualUserFragment::into(crate::context::RolloutBudgetContext {
        remaining_tokens: reminder.remaining_tokens,
    });
    sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
        .await;
    budget.mark_reminder_delivered(sess.thread_id(), window_id, reminder);
}

impl Session {
    pub(crate) async fn record_rollout_budget_usage(&self, usage: &TokenUsage) -> CodexResult<()> {
        let budget = self.services.agent_control.rollout_budget();
        let exhausted = budget.record_usage(usage)?;
        if let Some(checkpoint) = budget.checkpoint() {
            self.services
                .agent_control
                .persist_rollout_budget_checkpoint_for_other_threads(self.thread_id(), checkpoint)
                .await;
        }
        if exhausted {
            return Err(CodexErr::SessionBudgetExceeded);
        }
        Ok(())
    }

    pub(crate) async fn persist_current_rollout_budget_checkpoint(&self) {
        let Some(checkpoint) = self.services.agent_control.rollout_budget().checkpoint() else {
            return;
        };
        self.persist_rollout_items(&[RolloutItem::EventMsg(EventMsg::TokenCount(
            TokenCountEvent {
                info: None,
                rate_limits: None,
                rollout_budget: Some(checkpoint),
            },
        ))])
        .await;
    }

    pub(super) fn restore_rollout_budget(&self, rollout_items: &[RolloutItem]) {
        let checkpoint = rollout_items
            .iter()
            .filter_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::TokenCount(event)) => event.rollout_budget,
                _ => None,
            })
            .max_by(|left, right| {
                left.weighted_tokens_used
                    .total_cmp(&right.weighted_tokens_used)
            });
        if let Some(checkpoint) = checkpoint {
            self.services
                .agent_control
                .rollout_budget()
                .restore(checkpoint);
        }
    }
}
