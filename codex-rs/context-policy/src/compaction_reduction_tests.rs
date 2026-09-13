use super::*;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;

fn message(role: &str, text: impl Into<String>) -> ResponseItem {
    let text = text.into();
    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: vec![if role == "assistant" {
            ContentItem::OutputText { text }
        } else {
            ContentItem::InputText { text }
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[test]
fn context_rejection_reduces_multiple_complete_turn_groups_once() {
    let mut items = Vec::new();
    for index in 0..10 {
        items.push(message("user", format!("user-{index}")));
        items.push(message("assistant", format!("assistant-{index}")));
    }
    items.push(message("user", "compact this history"));
    let measured = items
        .iter()
        .map(|item| CompactionItem {
            item,
            estimated_tokens: 100,
            starts_turn: item.is_user_message(),
        })
        .collect::<Vec<_>>();

    let decision = plan_compaction_reduction(
        &measured,
        /*compaction_input_items*/ 1,
        /*base_tokens*/ 0,
        RequestBudget::ProviderEnforced,
    )
    .expect("the plan should remove old turns");

    assert_eq!(
        decision,
        CompactionReduction {
            retained_indices: (6..21).collect(),
            reduction: LocalCompactionReduction {
                removed_groups: 3,
                removed_items: 6,
                estimated_tokens_before: 2_100,
                estimated_tokens_after: 1_500,
                target_tokens: 1_680,
            },
        }
    );
    assert_eq!(
        plan_compaction_reduction(
            &measured[18..],
            /*compaction_input_items*/ 1,
            /*base_tokens*/ 0,
            RequestBudget::ProviderEnforced,
        ),
        Err(CompactionReductionExhausted)
    );
}

#[test]
fn context_rejection_reduces_complete_tool_exchanges_inside_the_latest_turn() {
    let mut items = vec![message("user", "investigate the failure")];
    for index in 0..6 {
        let call_id = format!("call-{index}");
        items.push(ResponseItem::FunctionCall {
            id: None,
            name: "exec_command".to_string(),
            namespace: None,
            arguments: format!(r#"{{"cmd":"step {index}"}}"#),
            encrypted_function_args: None,
            call_id: call_id.clone(),
            internal_chat_message_metadata_passthrough: None,
        });
        items.push(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("tool output ".repeat(300)),
            internal_chat_message_metadata_passthrough: None,
        });
    }
    items.push(message("user", "compact this history"));
    let measured = items
        .iter()
        .map(|item| CompactionItem {
            item,
            estimated_tokens: if matches!(item, ResponseItem::FunctionCallOutput { .. }) {
                500
            } else {
                20
            },
            starts_turn: item.is_user_message(),
        })
        .collect::<Vec<_>>();

    let decision = plan_compaction_reduction(
        &measured,
        /*compaction_input_items*/ 1,
        /*base_tokens*/ 0,
        RequestBudget::ClientEnforced(2_000),
    )
    .expect("latest-turn tool exchanges should be reducible");

    assert_eq!(
        decision,
        CompactionReduction {
            retained_indices: vec![0, 7, 8, 9, 10, 11, 12, 13],
            reduction: LocalCompactionReduction {
                removed_groups: 0,
                removed_items: 6,
                estimated_tokens_before: 3_160,
                estimated_tokens_after: 1_600,
                target_tokens: 1_600,
            },
        }
    );
    let retained = decision
        .retained_indices
        .iter()
        .map(|index| &items[*index])
        .collect::<Vec<_>>();
    let calls = retained
        .iter()
        .filter_map(|item| tool_call_key(item))
        .collect::<std::collections::HashSet<_>>();
    let outputs = retained
        .iter()
        .filter_map(|item| tool_output_key(item))
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(calls, outputs);
}
