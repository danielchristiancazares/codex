use super::*;
use crate::session::tests::make_session_and_context;
use codex_utils_output_truncation::CaptureQuery;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;
use std::num::NonZeroUsize;
use std::time::Duration;

#[tokio::test]
async fn failed_terminal_capture_preserves_nested_output_and_hook_payloads() {
    let (session, _) = make_session_and_context().await;
    let marker = "DENIAL-MIDDLE-EVIDENCE";
    let output = ExecCommandToolOutput {
        event_call_id: "denied-command".to_string(),
        chunk_id: "denied-chunk".to_string(),
        wall_time: Duration::from_millis(10),
        raw_output: format!(
            "{}{marker}{}",
            "before\n".repeat(100),
            "\nafter".repeat(100)
        )
        .into_bytes(),
        truncation_policy: TruncationPolicy::Tokens(100),
        max_output_tokens: Some(100),
        process_id: None,
        exit_code: Some(1),
        original_token_count: None,
        output_omitted_bytes: NonZeroUsize::new(17),
        hook_command: Some("denied command".to_string()),
    };
    let payload = ToolPayload::Function {
        arguments: r#"{"cmd":"denied command"}"#.to_string(),
    };
    let expected_nested = output.code_mode_result(&payload);
    let expected_hook = output.post_tool_use_response("denied-command", &payload);
    let expected_input = output.post_tool_use_input(&payload);
    let captured = CapturedOutput::terminal(&session, output).await;
    let mut nested = captured.code_mode_result(&payload);
    let fields = nested.as_object_mut().unwrap();
    let id = fields.remove("output_id").unwrap();
    uuid::Uuid::parse_str(id.as_str().unwrap()).unwrap();
    assert!(
        fields
            .remove("output_capture")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("17 bytes omitted")
    );
    assert_eq!(nested, expected_nested);
    assert!(!nested["output"].as_str().unwrap().contains(marker));
    assert_eq!(
        captured.post_tool_use_response("denied-command", &payload),
        expected_hook
    );
    assert_eq!(captured.post_tool_use_input(&payload), expected_input);
    assert_eq!(captured.post_tool_use_id("poll"), "denied-command");
    let recovered = session
        .services
        .captured_output
        .lock()
        .await
        .read(
            captured.receipt.id(),
            CaptureQuery::Search(marker.to_string().try_into().unwrap()),
        )
        .unwrap();
    assert!(recovered.contains(marker));
}
