use super::*;
use codex_protocol::models::ContentItem;
use codex_protocol::openai_models::default_input_modalities;
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

fn base_instructions() -> BaseInstructions {
    BaseInstructions {
        text: String::new(),
        provenance: None,
    }
}

fn plan_with_items(items: &[ResponseItem]) -> LocalCompactionPlan {
    let mut history = ContextManager::new();
    history.record_items(items, TruncationPolicy::Tokens(100_000));
    LocalCompactionPlan::new(
        history,
        message("user", "compact this history"),
        TruncationPolicy::Tokens(100_000),
    )
}

#[test]
fn context_rejection_reduces_multiple_complete_turn_groups_once() {
    let mut items = Vec::new();
    for index in 0..10 {
        items.push(message("user", format!("user-{index}-{}", "u".repeat(400))));
        items.push(message(
            "assistant",
            format!("assistant-{index}-{}", "a".repeat(400)),
        ));
    }
    let mut plan = plan_with_items(&items);
    let before = plan.prompt_input(
        &default_input_modalities(),
        TruncationPolicy::Tokens(100_000),
    );

    let reduction = plan
        .reduce_after_context_error(&base_instructions(), /*context_window*/ None)
        .expect("the plan should remove old turns");
    let after = plan.prompt_input(
        &default_input_modalities(),
        TruncationPolicy::Tokens(100_000),
    );

    assert!(reduction.removed_groups >= 2);
    assert!(reduction.removed_items >= 4);
    assert_eq!(after.last(), before.last());
    assert_eq!(
        plan.reduce_after_context_error(&base_instructions(), /*context_window*/ None),
        None
    );
}

#[test]
fn replacement_budget_charges_media_only_and_tiny_message_envelopes() {
    let mut items = Vec::new();
    for index in 0..200 {
        items.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: format!("data:image/png;base64,{index}"),
                detail: None,
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        });
        items.push(message("user", "x"));
    }
    let plan = plan_with_items(&items);

    let replacement = plan
        .build_replacement(
            Vec::new(),
            "bounded summary",
            &base_instructions(),
            Some(400),
            CompactedMessageIdentity::Regenerate,
        )
        .expect("a bounded replacement should fit");

    assert!(estimated_request_tokens(&base_instructions(), &replacement.items) <= 400);
    assert!(replacement.items.len() < items.len());
}

#[test]
fn replacement_budget_truncates_an_oversized_summary() {
    let plan = plan_with_items(&[message("user", "retain me if space allows")]);
    let summary = "summary ".repeat(2_000);

    let replacement = plan
        .build_replacement(
            Vec::new(),
            &summary,
            &base_instructions(),
            Some(200),
            CompactedMessageIdentity::Regenerate,
        )
        .expect("the summary should be reduced to fit");

    assert!(replacement.summary_text.len() < summary.len());
    assert!(estimated_request_tokens(&base_instructions(), &replacement.items) <= 200);
}
