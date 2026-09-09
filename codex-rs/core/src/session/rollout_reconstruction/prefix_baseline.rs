//! Restores the canonical prefix that a complete rollback leaves in history.

use super::TurnReferenceContextItem;
use super::is_user_turn_boundary;
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_protocol::protocol::EventMsg;

impl TurnReferenceContextItem {
    pub(super) fn restore_retained_prefix<'a>(
        &mut self,
        rollout: &'a [RolloutItem],
        retained: &[ResponseItemEnvelope],
        world_state_replay: &mut Vec<&'a RolloutItem>,
    ) {
        match self {
            Self::Latest(_) | Self::Cleared => return,
            Self::NeverSet => {}
        }
        // Compaction changes prefix provenance. Its surviving checkpoint remains
        // authoritative, and ordinary context-only forks use the reverse replay.
        if retained.is_empty()
            || retained
                .iter()
                .any(|item| is_user_turn_boundary(&item.item))
            || rollout
                .iter()
                .any(|item| matches!(item, RolloutItem::Compacted(_)))
            || !rollout
                .iter()
                .any(|item| matches!(item, RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_))))
        {
            return;
        }
        let prefix_len = rollout
            .iter()
            .position(|item| match item {
                RolloutItem::ResponseItem(item) => is_user_turn_boundary(&item.item),
                RolloutItem::InterAgentCommunication(_) => true,
                _ => false,
            })
            .unwrap_or(rollout.len());
        let prefix = &rollout[..prefix_len];
        if let Some(context) = prefix.iter().rev().find_map(|item| match item {
            RolloutItem::TurnContext(context) => Some(context),
            _ => None,
        }) {
            *self = Self::Latest(Box::new(context.clone()));
            for item in prefix
                .iter()
                .rev()
                .filter(|item| matches!(item, RolloutItem::WorldState(_)))
            {
                if !world_state_replay
                    .iter()
                    .any(|seen| std::ptr::eq(*seen, item))
                {
                    world_state_replay.push(item);
                }
            }
        }
    }
}
