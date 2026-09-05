use codex_extension_api::ToolName;
use codex_extension_api::ToolPayload;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::user_input::UserInput;
use serde_json::json;

use super::MAX_ASSEMBLED_CONTEXT_TOKENS;
use super::OMISSION_NOTICE;
use super::bound_model_context;
use super::bound_user_inputs;
use super::prepare_model_context;
use super::serialized_bytes;
use super::serialized_text_item_bytes;
use crate::async_scorer::action::GuardianAction;

#[test]
fn assembled_context_bounds_all_sections_and_serialization_framing() {
    let prefix = vec![
        "authorization\n".to_string(),
        ">>> TRANSCRIPT START\n".to_string(),
    ];
    let entries = vec![
        format!("oldest {}\n", "a".repeat(/*n*/ 20_000)),
        format!("newest {}\n", "b".repeat(/*n*/ 20_000)),
    ];
    let suffix = vec![
        ">>> TRANSCRIPT END\n".to_string(),
        ">>> APPROVAL REQUEST START\n".to_string(),
        "{\"tool\":\"exec\"}\n".to_string(),
        ">>> APPROVAL REQUEST END\n".to_string(),
    ];

    let bounded = bound_model_context(prefix, entries, vec![1], suffix).expect("bounded context");
    assert!(bounded.original_bytes > bounded.retained_bytes);
    assert!(
        bounded.retained_bytes
            <= TruncationPolicy::Tokens(MAX_ASSEMBLED_CONTEXT_TOKENS).byte_budget()
    );
    assert!(bounded.items.iter().any(|item| item == OMISSION_NOTICE));
    assert!(!bounded.items.iter().any(|item| item.starts_with("oldest ")));
    assert!(bounded.items.iter().any(|item| item.starts_with("newest ")));
    assert_eq!(
        serialized_bytes(&bounded.items).expect("serialized context"),
        bounded.retained_bytes
    );
}

#[test]
fn prepared_context_preserves_mandatory_anchors_and_exactly_bounds_escaped_action() {
    let entries = vec![
        format!("[1] user: first authorization {}\n", "a".repeat(3_000)),
        format!("[2] tool call: optional evidence {}\n", "b".repeat(30_000)),
        format!("[3] user: latest authorization {}\n", "c".repeat(3_000)),
        format!("[4] developer: protected approval {}\n", "d".repeat(3_000)),
    ];
    let action = GuardianAction {
        tool_name: ToolName::plain("send_file"),
        payload: ToolPayload::Function {
            arguments: json!({
                "attachments": [{
                    "content": "🦀\"\\\n".repeat(20_000),
                    "name": "financials.csv",
                }],
                "call_id": "genuine-call",
                "path": "🦀\"\\\n".repeat(20_000),
            })
            .to_string(),
        },
    };

    let prepared = prepare_model_context(
        vec!["authorization\n".to_owned()],
        entries.clone(),
        vec![0, 2, 3],
        action,
        /*max_action_tokens*/ MAX_ASSEMBLED_CONTEXT_TOKENS,
    )
    .expect("prepared context");
    let max_bytes = TruncationPolicy::Tokens(MAX_ASSEMBLED_CONTEXT_TOKENS).byte_budget();
    assert_eq!(
        serialized_bytes(&prepared.items).expect("serialized context"),
        prepared.retained_bytes
    );
    assert!(prepared.retained_bytes <= max_bytes);
    assert!(prepared.items.contains(&entries[0]));
    assert!(!prepared.items.contains(&entries[1]));
    assert!(prepared.items.contains(&entries[2]));
    assert!(prepared.items.contains(&entries[3]));
    assert!(prepared.items.iter().any(|item| item == OMISSION_NOTICE));
    assert!(
        prepared
            .items
            .iter()
            .any(|item| item == &format!("{}\n", prepared.action.text))
    );

    let action_value =
        serde_json::from_str::<serde_json::Value>(&prepared.action.text).expect("action JSON");
    assert_eq!(action_value["tool"], "send_file");
    assert_eq!(action_value["call_id"], "genuine-call");
    assert!(
        action_value["path"]
            .as_str()
            .is_some_and(|path| path.contains("<truncated omitted_approx_tokens=\""))
    );

    let action_item = format!("{}\n", prepared.action.text);
    let action_index = prepared
        .items
        .iter()
        .position(|item| item == &action_item)
        .expect("action item");
    let mut placeholder_items = prepared.items.clone();
    placeholder_items[action_index] = "\n".to_owned();
    let placeholder_context_bytes =
        serialized_bytes(&placeholder_items).expect("placeholder context");
    let placeholder_item_bytes = serialized_text_item_bytes("\n").expect("placeholder item");
    let available_action_item_bytes =
        max_bytes - (placeholder_context_bytes - placeholder_item_bytes);
    let action_item_bytes =
        serialized_text_item_bytes(&action_item).expect("serialized action item");
    assert!(action_item_bytes <= available_action_item_bytes);
    assert!(
        available_action_item_bytes.saturating_sub(action_item_bytes) < 256,
        "water-filling should use the exact JSON-encoded action capacity"
    );
    assert!(action_item_bytes > action_item.len());
}

#[test]
fn assembled_context_fails_closed_when_required_evidence_cannot_fit() {
    let error = bound_model_context(
        vec!["authorization".to_owned()],
        vec![
            "required anchor ".to_owned()
                + &"x".repeat(TruncationPolicy::Tokens(MAX_ASSEMBLED_CONTEXT_TOKENS).byte_budget()),
        ],
        vec![0],
        vec!["approval".to_string()],
    )
    .err();
    assert_eq!(
        error.as_deref(),
        Some(
            "Guardian authorization, required transcript anchors, and approval request exceed the 10000-token safety limit"
        )
    );
}

#[test]
fn user_input_bound_preserves_non_text_evidence_and_required_suffix() {
    let image = UserInput::Image {
        image_url: "data:image/png;base64,image".to_string(),
        detail: None,
    };
    let items = vec![
        UserInput::Text {
            text: "authorization\n".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Text {
            text: "oldest ".to_string() + &"a".repeat(/*n*/ 40_000),
            text_elements: Vec::new(),
        },
        image.clone(),
        UserInput::Text {
            text: ">>> APPROVAL REQUEST END\n".to_string(),
            text_elements: Vec::new(),
        },
    ];

    let bounded = bound_user_inputs(items, 1..2, &[]).expect("bounded user inputs");
    assert!(bounded.contains(&image));
    assert!(
        bounded
            .iter()
            .any(|item| matches!(item, UserInput::Text { text, .. } if text == OMISSION_NOTICE))
    );
    assert!(bounded.iter().any(
        |item| matches!(item, UserInput::Text { text, .. } if text == ">>> APPROVAL REQUEST END\n")
    ));
}
