use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextKind;

use crate::compact::insert_initial_context_before_last_real_user_or_summary;

use super::Session;

impl Session {
    pub(crate) async fn rehydrate_additional_context_for_compaction(
        &self,
        replacement: Vec<ResponseItemEnvelope>,
    ) -> Vec<ResponseItemEnvelope> {
        let (current, history) = {
            let state = self.state.lock().await;
            (
                state.additional_context.current_keys_and_kinds(),
                state.history.annotated_items().to_vec(),
            )
        };
        let retained = current
            .into_iter()
            .filter_map(|(key, kind)| {
                let content_kind = format!("additional_content.{key}");
                if replacement
                    .iter()
                    .any(|item| additional_context_item_matches(item, &content_kind, kind))
                {
                    return None;
                }
                history
                    .iter()
                    .rev()
                    .find(|item| additional_context_item_matches(item, &content_kind, kind))
                    .cloned()
            })
            .collect::<Vec<_>>();
        if retained.is_empty() {
            replacement
        } else {
            insert_initial_context_before_last_real_user_or_summary(replacement, retained)
        }
    }
}

fn additional_context_item_matches(
    item: &ResponseItemEnvelope,
    content_kind: &str,
    kind: AdditionalContextKind,
) -> bool {
    let expected_role = match kind {
        AdditionalContextKind::Untrusted => "user",
        AdditionalContextKind::Application => "developer",
    };
    matches!(
        &item.item,
        ResponseItem::Message {
            role,
            internal_chat_message_metadata_passthrough: Some(metadata),
            ..
        } if role == expected_role
            && metadata.content_item_kinds.as_ref().is_some_and(|kinds| {
                kinds.iter().any(|kind| kind.0 == content_kind)
            })
    )
}
