use super::test_model_client;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;

fn output_items(call_id: &str, text: &str) -> [ResponseItem; 2] {
    [
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id.to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text(text.to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::CustomToolCallOutput {
            id: None,
            call_id: call_id.to_string(),
            name: None,
            output: FunctionCallOutputPayload::from_text(text.to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
    ]
}

#[test]
fn request_preparation_preserves_bounded_function_output_envelopes() {
    let client = test_model_client(SessionSource::Exec);
    let expected = output_items("call", "output");
    let mut actual = expected.clone();
    assert!(
        client
            .prepare_response_items_for_request(&mut actual)
            .is_ok()
    );
    assert_eq!(actual, expected);
}

#[test]
fn request_preparation_rejects_oversized_function_output_envelopes() {
    let client = test_model_client(SessionSource::Exec);
    for (call_id, text) in [
        ("call".to_string(), "!@#$%^&*()[]{}".repeat(4_000)),
        ("!@#$%^&*()[]{}".repeat(4_000), String::new()),
    ] {
        for item in output_items(&call_id, &text) {
            let error = client
                .prepare_response_items_for_request(&mut [item])
                .expect_err("oversized tool output must fail request preparation");
            assert!(
                matches!(error.details(), CodexErrorDetails::InvalidRequest(message)
                if message == "tool output exceeds the 10K-token model-context item budget")
            );
            assert!(!error.is_retryable());
        }
    }
}
