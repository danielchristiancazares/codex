//! Retains accepted source publications when compaction replaces conversation history.

use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextKind;

use crate::compact::insert_initial_context_before_last_real_user_or_summary;
use crate::state::AdditionalContextSnapshot;

use super::Session;

impl Session {
    pub(crate) async fn rehydrate_additional_context_for_compaction(
        &self,
        replacement: Vec<ResponseItemEnvelope>,
    ) -> (Vec<ResponseItemEnvelope>, AdditionalContextSnapshot) {
        let (current, snapshot, history, pending_search) = {
            let state = self.state.lock().await;
            (
                state.additional_context.current_keys_and_kinds(),
                state.additional_context.snapshot(),
                state.history.annotated_items().to_vec(),
                state.history.pending_tool_search_exchange(),
            )
        };
        let retained = current
            .into_iter()
            .filter_map(|(key, kind)| {
                let content_kind = format!("additional_content.{key}");
                let expected_role = match kind {
                    AdditionalContextKind::Untrusted => "user",
                    AdditionalContextKind::Application => "developer",
                };
                let matches_publication = |item: &&ResponseItemEnvelope| {
                    matches!(
                        &item.item,
                        ResponseItem::Message {
                            role,
                            internal_chat_message_metadata_passthrough: Some(metadata),
                            ..
                        } if role == expected_role && metadata.content_item_kinds.as_ref()
                            .is_some_and(|kinds| kinds.iter().any(|kind| kind.0 == content_kind))
                    )
                };
                match replacement.iter().find(matches_publication) {
                    Some(_) => None,
                    None => history.iter().rev().find(matches_publication).cloned(),
                }
            })
            .collect::<Vec<_>>();
        let mut replacement =
            insert_initial_context_before_last_real_user_or_summary(replacement, retained);
        // A complete pair carries the unconsumed schemas. Remove only this matching
        // exchange before reinserting it ahead of the terminal summary/checkpoint.
        if let [call, output] = pending_search.as_slice() {
            replacement.retain(|item| match (&item.item, &call.item, &output.item) {
                (
                    ResponseItem::ToolSearchCall {
                        call_id: Some(id), ..
                    },
                    ResponseItem::ToolSearchCall {
                        call_id: Some(expected),
                        ..
                    },
                    _,
                ) => id != expected,
                (
                    ResponseItem::ToolSearchOutput {
                        call_id: Some(id), ..
                    },
                    _,
                    ResponseItem::ToolSearchOutput {
                        call_id: Some(expected),
                        ..
                    },
                ) => id != expected,
                _ => true,
            });
            let summary_index = replacement
                .iter()
                .rposition(|item| {
                    matches!(
                        &item.item, ResponseItem::Message { role, content, .. }
                        if role == "user" && content.iter().any(|content| matches!(content,
                            codex_protocol::models::ContentItem::InputText { text }
                            if text.starts_with(crate::compact::SUMMARY_PREFIX)
                        ))
                    )
                })
                .unwrap_or(replacement.len());
            replacement.splice(summary_index..summary_index, pending_search);
        }
        (replacement, snapshot)
    }

    /// Checks the replacement together with publications and pending tool exchanges.
    /// The client also checks the final request after tool/envelope formatting.
    pub(crate) async fn fit_compaction_replacement(
        &self,
        turn: &super::turn_context::TurnContext,
        items: Vec<ResponseItemEnvelope>,
    ) -> codex_protocol::error::Result<Vec<ResponseItemEnvelope>> {
        let (items, _) = self
            .rehydrate_additional_context_for_compaction(items)
            .await;
        let base = self.get_prompt_base_instructions().await;
        let mut projected = crate::context_manager::ContextManager::new();
        projected.replace_annotated(items.clone());
        let tokens = projected
            .for_prompt(&turn.model_info().input_modalities)
            .iter()
            .map(crate::context_manager::estimate_item_token_count)
            .fold(
                i64::try_from(codex_utils_output_truncation::approx_token_count(
                    &base.text,
                ))
                .unwrap_or(i64::MAX),
                i64::saturating_add,
            );
        crate::context_manager::RequestBudget::for_model(turn.model_info())
            .check(tokens)
            .map_err(|error| {
                codex_protocol::error::CodexErr::Fatal(format!(
                    "compaction replacement cannot fit: {error}"
                ))
            })?;
        Ok(items)
    }
}
