use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::user_input::UserInput;

use super::MAX_ASSEMBLED_GUARDIAN_CONTEXT_TOKENS;
use super::OMISSION_NOTICE;
use super::bound_guardian_prompt_items;
use super::serialized_text_bytes;

fn text(value: impl Into<String>) -> UserInput {
    UserInput::Text {
        text: value.into(),
        text_elements: Vec::new(),
    }
}

#[test]
fn bounds_complete_prompt_with_framing_and_keeps_required_suffix() {
    let items = vec![
        text("authorization\n"),
        text(">>> TRANSCRIPT START\n"),
        text(format!("first user {}\n", "a".repeat(/*n*/ 4_000))),
        text(format!("old optional {}\n", "b".repeat(/*n*/ 30_000))),
        text(format!("latest user {}\n", "c".repeat(/*n*/ 4_000))),
        text(format!("protected message {}\n", "d".repeat(/*n*/ 4_000))),
        text(">>> TRANSCRIPT END\n"),
        text(">>> APPROVAL REQUEST START\n"),
        text("{\"tool\":\"exec\"}\n"),
        text(">>> APPROVAL REQUEST END\n"),
    ];

    let bounded = bound_guardian_prompt_items(items, 2..6, &[0, 2, 3]).expect("bounded prompt");
    assert!(
        serialized_text_bytes(&bounded).expect("serialized prompt")
            <= TruncationPolicy::Tokens(MAX_ASSEMBLED_GUARDIAN_CONTEXT_TOKENS).byte_budget()
    );
    assert!(
        bounded
            .iter()
            .any(|item| matches!(item, UserInput::Text { text, .. } if text == OMISSION_NOTICE))
    );
    assert!(
        bounded
            .iter()
            .any(|item| matches!(item, UserInput::Text { text, .. } if text == "authorization\n"))
    );
    assert!(bounded.iter().any(
        |item| matches!(item, UserInput::Text { text, .. } if text.starts_with("first user "))
    ));
    assert!(!bounded.iter().any(
        |item| matches!(item, UserInput::Text { text, .. } if text.starts_with("old optional "))
    ));
    assert!(bounded.iter().any(
        |item| matches!(item, UserInput::Text { text, .. } if text.starts_with("latest user "))
    ));
    assert!(bounded.iter().any(
        |item| matches!(item, UserInput::Text { text, .. } if text.starts_with("protected message "))
    ));
    assert!(bounded.iter().any(
        |item| matches!(item, UserInput::Text { text, .. } if text == ">>> APPROVAL REQUEST END\n")
    ));
}

#[test]
fn fails_closed_when_required_context_alone_is_oversized() {
    let error = bound_guardian_prompt_items(
        vec![
            text("authorization\n"),
            text(
                "required anchor ".to_owned()
                    + &"x".repeat(
                        TruncationPolicy::Tokens(MAX_ASSEMBLED_GUARDIAN_CONTEXT_TOKENS)
                            .byte_budget(),
                    ),
            ),
            text("approval\n"),
        ],
        1..2,
        &[0],
    )
    .err();
    assert_eq!(
        error.map(|error| error.to_string()).as_deref(),
        Some(
            "Guardian authorization, required transcript anchors, and approval request exceed the 10000-token safety limit"
        )
    );
}
