use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use codex_history::RolloutItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

use crate::state::SessionState;

const CURRENT_TIME_REMINDER_KIND: &str = "current_time.reminder";
const TOKEN_BUDGET_REMINDER_KIND: &str = "token_budget.reminder";
const CURRENT_TIME_PREFIX: &str = "<current_time_reminder>It is ";
const CURRENT_TIME_SUFFIX: &str = " UTC.</current_time_reminder>";

pub(super) struct ReminderDelivery {
    current_time: Option<DateTime<Utc>>,
    token_budget_delivered: bool,
}

impl ReminderDelivery {
    pub(super) fn from_rollout(rollout_items: &[RolloutItem]) -> Self {
        let active_window = rollout_items
            .iter()
            .rposition(|item| matches!(item, RolloutItem::Compacted(_)))
            .map_or(rollout_items, |index| &rollout_items[index + 1..]);
        let mut current_time = None;
        let mut token_budget_delivered = false;
        for item in active_window {
            let RolloutItem::ResponseItem(envelope) = item else {
                continue;
            };
            let ResponseItem::Message {
                content,
                internal_chat_message_metadata_passthrough: Some(metadata),
                ..
            } = &envelope.item
            else {
                continue;
            };
            let Some(kinds) = metadata
                .content_item_kinds
                .as_ref()
                .filter(|kinds| kinds.len() == content.len())
            else {
                continue;
            };
            for (content, kind) in content.iter().zip(kinds) {
                match kind.0.as_str() {
                    CURRENT_TIME_REMINDER_KIND => {
                        if let Some(delivery_time) = parse_current_time_reminder(content) {
                            current_time = Some(delivery_time);
                        }
                    }
                    TOKEN_BUDGET_REMINDER_KIND => token_budget_delivered = true,
                    _ => {}
                }
            }
        }
        Self {
            current_time,
            token_budget_delivered,
        }
    }

    pub(super) fn restore(self, state: &mut SessionState, window_id: &str) {
        if let Some(current_time) = self.current_time {
            state
                .current_time_reminder
                .restore_delivery(window_id, current_time);
        }
        if self.token_budget_delivered {
            state.restore_token_budget_reminder_delivered();
        }
    }
}

fn parse_current_time_reminder(content: &ContentItem) -> Option<DateTime<Utc>> {
    let ContentItem::InputText { text } = content else {
        return None;
    };
    let timestamp = text
        .strip_prefix(CURRENT_TIME_PREFIX)?
        .strip_suffix(CURRENT_TIME_SUFFIX)?;
    NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|timestamp| timestamp.and_utc())
}
