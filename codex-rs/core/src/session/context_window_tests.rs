use super::context_window_token_status;
use crate::session::tests::make_session_and_context;
use crate::session::tests::update_turn_settings_for_test;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recounted_context_usage_is_preserved_after_filtering_history() {
    let (session, mut turn_context) = make_session_and_context().await;
    update_turn_settings_for_test(&mut turn_context, |settings| {
        Arc::make_mut(&mut settings.model_info).input_modalities = vec![InputModality::Text];
    });
    let items = [
        json!({
            "type": "message", "role": "user",
            "content": [
                {"type": "input_text", "text": "Describe the attachment."},
                {"type": "input_image", "image_url": "data:image/png;base64,AAA"},
                {"type": "input_audio", "audio_url": "data:audio/wav;base64,AAA"},
            ],
        }),
        json!({
            "type": "message", "role": "assistant",
            "content": [{"type": "output_text", "text":
                format!("Answer.<oai-mem-citation>{}</oai-mem-citation>", "citation".repeat(/*n*/ 400))
            }],
        }),
    ]
    .into_iter()
    .map(serde_json::from_value)
    .collect::<serde_json::Result<Vec<ResponseItem>>>()
    .unwrap();
    session
        .state
        .lock()
        .await
        .record_items(&items, TruncationPolicy::Tokens(10_000));
    session.recompute_token_usage(&turn_context).await;
    let recounted = session
        .state
        .lock()
        .await
        .token_info()
        .expect("recounted usage");
    assert!(recounted.last_token_usage.total_tokens > 0);

    let status = context_window_token_status(&session, &turn_context).await;
    assert_eq!(
        status.active_context_tokens,
        recounted.last_token_usage.total_tokens
    );
    assert_eq!(session.state.lock().await.token_info(), Some(recounted));
}

#[tokio::test]
async fn context_usage_projects_new_tool_output_without_discounting_reported_usage() {
    let (session, mut turn_context) = make_session_and_context().await;
    update_turn_settings_for_test(&mut turn_context, |settings| {
        Arc::make_mut(&mut settings.model_info).input_modalities = vec![InputModality::Text];
    });
    {
        let mut state = session.state.lock().await;
        let message: ResponseItem = serde_json::from_value(json!({
            "type": "message", "role": "assistant",
            "content": [{"type": "output_text", "text":
                "Answer.<oai-mem-citation>hidden reference</oai-mem-citation>"
            }],
        }))
        .unwrap();
        state.record_items(&[message], TruncationPolicy::Tokens(10_000));
        state.set_token_info(Some(TokenUsageInfo {
            total_token_usage: TokenUsage::default(),
            last_token_usage: TokenUsage {
                total_tokens: 5_000,
                ..TokenUsage::default()
            },
            model_context_window: None,
        }));
        let output: ResponseItem = serde_json::from_value(json!({
            "type": "function_call_output", "name": "external",
            "output": [
                {"type": "input_text", "text": "tool output"},
                {"type": "input_image", "image_url": "data:image/png;base64,AAA"},
            ],
        }))
        .unwrap();
        state.record_items(&[output], TruncationPolicy::Tokens(10_000));
    }

    let status = context_window_token_status(&session, &turn_context).await;
    // The text-only projection keeps the tool text and the unsupported-image notice.
    assert_eq!(status.active_context_tokens, 5_020);
}
