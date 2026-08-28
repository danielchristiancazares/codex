use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolOutput;
use codex_tools::ToolPayload;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::HistoryNotesToolOutput;

#[test]
fn preserves_encrypted_history_output() {
    for encrypted_content in ["enc_payload".to_string(), "x".repeat(/*n*/ 40_001)] {
        let result = HistoryNotesToolOutput::new(json!({"encrypted_output": encrypted_content}))
            .expect("valid encrypted output")
            .to_response_item(
                "call-1",
                &ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
            );

        let ResponseInputItem::FunctionCallOutput { output, .. } = result else {
            panic!("expected function-call output");
        };
        assert_eq!(
            output.content_items(),
            Some(
                [FunctionCallOutputContentItem::EncryptedContent { encrypted_content }].as_slice()
            )
        );
    }
}

#[test]
fn preserves_complete_plaintext_json_within_the_safety_limit() {
    let result = json!({"content": "x".repeat(/*n*/ 39_986)});
    let output = HistoryNotesToolOutput::new(result.clone()).expect("bounded plaintext");
    assert_eq!(
        output.to_response_item(
            "call-1",
            &ToolPayload::Function {
                arguments: "{}".to_string()
            },
        ),
        ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text(result.to_string()),
        },
    );
}

#[test]
fn rejects_plaintext_above_the_safety_limit() {
    let result = HistoryNotesToolOutput::new(json!({"content": "x".repeat(/*n*/ 40_001)}));
    assert_eq!(
        result.err(),
        Some(codex_tools::FunctionCallError::RespondToModel(
            "History returned plaintext above the 10000-token safety limit; retry with narrower bounds"
                .to_string(),
        )),
    );
}
