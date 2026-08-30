use super::*;
use codex_history::CodexHarnessMetadata;

#[test]
fn rewritten_output_preserves_harness_metadata() {
    let envelope = ResponseItemEnvelope {
        item: ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text("large output".repeat(100)),
                success: Some(true),
            },
            internal_chat_message_metadata_passthrough: None,
        },
        metadata: Some(CodexHarnessMetadata::default()),
    };

    let rewritten = rewritten_output_for_context_window(&envelope)
        .expect("function output should be rewritten");

    assert_eq!(rewritten.metadata, envelope.metadata);
    assert_ne!(rewritten.item, envelope.item);
}

#[test]
fn shared_trimmer_reaches_outputs_behind_newer_message_groups() {
    let old_output = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: Some("call-1".to_string()),
        name: None,
        namespace: None,
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text("large output".repeat(2_000)),
            success: Some(true),
        },
        internal_chat_message_metadata_passthrough: None,
    };
    let newer_message = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![codex_protocol::models::ContentItem::InputText {
            text: "newer boundary".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let expected_output = ResponseItem::FunctionCallOutput {
        id: None,
        call_id: Some("call-1".to_string()),
        name: None,
        namespace: None,
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text(CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE.to_string()),
            success: Some(true),
        },
        internal_chat_message_metadata_passthrough: None,
    };
    let mut history = ContextManager::new();
    history.replace_annotated(vec![
        ResponseItemEnvelope::new(old_output),
        ResponseItemEnvelope::new(newer_message.clone()),
    ]);
    let base_instructions = BaseInstructions {
        text: String::new(),
        provenance: None,
    };

    let (rewritten_outputs, deleted_tokens) = trim_function_call_history_for_context_window(
        &mut history,
        Some(/*context_window*/ 100),
        &base_instructions,
    );

    assert_eq!(rewritten_outputs, 1);
    assert!(deleted_tokens > 0);
    assert_eq!(
        history.raw_items().cloned().collect::<Vec<_>>(),
        vec![expected_output, newer_message]
    );
}
