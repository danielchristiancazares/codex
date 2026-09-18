use super::ContextManager;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use pretty_assertions::assert_eq;

#[test]
fn live_projection_preserves_pending_calls_and_filters_unsupported_images() {
    let pending = ResponseItemEnvelope::new(ResponseItem::CustomToolCall {
        id: None,
        status: None,
        call_id: "pending".to_string(),
        name: "exec".to_string(),
        namespace: None,
        input: "await tools.get_context_remaining({})".to_string(),
        internal_chat_message_metadata_passthrough: None,
    });
    let completed = ResponseItemEnvelope {
        item: ResponseItem::FunctionCallOutput {
            id: None,
            call_id: None,
            name: Some("external_tool".to_string()),
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "completed output".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AAA".to_string(),
                    detail: None,
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
        metadata: Some(CodexHarnessMetadata {
            history_truncation_token_limit: Some(32),
            ..Default::default()
        }),
    };
    let original = vec![completed.clone(), pending.clone()];
    let mut history = ContextManager::new();
    history.replace_annotated(original.clone());
    let live_delta = history.model_visible_token_delta(&[InputModality::Text]);

    let mut completed_history = ContextManager::new();
    completed_history.replace_annotated(vec![completed]);
    let completed_delta = completed_history.model_visible_token_delta(&[InputModality::Text]);
    assert!(completed_delta < 0);
    assert_eq!(live_delta, completed_delta);
    assert_eq!(history.annotated_items(), original);

    let mut pending_history = ContextManager::new();
    pending_history.replace_annotated(vec![pending]);
    assert_eq!(
        pending_history.model_visible_token_delta(&[InputModality::Text]),
        0
    );
}
