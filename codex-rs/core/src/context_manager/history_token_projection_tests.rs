use super::ContextManager;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;

#[test]
fn live_projection_preserves_pending_calls_and_bounds_completed_outputs() {
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
            output: FunctionCallOutputPayload::from_text("completed output ".repeat(1_000)),
            internal_chat_message_metadata_passthrough: None,
        },
        metadata: Some(CodexHarnessMetadata {
            fallback_token_limit_override: Some(32),
            ..Default::default()
        }),
    };
    let original = vec![completed.clone(), pending.clone()];
    let mut history = ContextManager::new();
    history.replace_annotated(original.clone());
    let policy = TruncationPolicy::Tokens(256);
    let live_delta = history.model_visible_token_delta(&[InputModality::Text], policy);

    let mut completed_history = ContextManager::new();
    completed_history.replace_annotated(vec![completed]);
    let completed_delta =
        completed_history.model_visible_token_delta(&[InputModality::Text], policy);
    assert!(completed_delta < 0);
    assert_eq!(live_delta, completed_delta);
    assert_eq!(history.annotated_items(), original);

    let mut pending_history = ContextManager::new();
    pending_history.replace_annotated(vec![pending]);
    assert_eq!(
        pending_history.model_visible_token_delta(&[InputModality::Text], policy),
        0
    );
}
