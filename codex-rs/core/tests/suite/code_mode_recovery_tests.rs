use super::*;
use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_recovers_outer_truncation_without_repeating_the_cell() -> Result<()> {
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
        })
        .build_with_auto_env(&server)
        .await?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("outer-1"),
            ev_custom_tool_call(
                "outer-call",
                "exec",
                r#"// @exec: {"max_output_tokens": 100}
store("recovery-executions", 1);
text("prefix\n".repeat(10000) + "OUTER-MIDDLE-EVIDENCE\n" + "suffix\n".repeat(10000));
"#,
            ),
            ev_completed("outer-1"),
        ]),
    )
    .await;
    let first_done = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("outer-done", "captured"),
            ev_completed("outer-2"),
        ]),
    )
    .await;
    test.submit_turn("Produce the large evidence once.").await?;
    let first = first_done.single_request();
    let output: FunctionCallOutputPayload =
        serde_json::from_value(first.custom_tool_call_output("outer-call")["output"].clone())?;
    let preview = output.body.to_text().expect("outer preview");
    assert!(!preview.contains("OUTER-MIDDLE-EVIDENCE"));
    let id = preview
        .strip_prefix("Captured output ")
        .expect("capture receipt")
        .split('.')
        .next()
        .unwrap();
    let source = format!(
        "text(await tools.read_output({})); text(load(\"recovery-executions\"));",
        json!({"output_id": id, "query": "search", "text": "OUTER-MIDDLE-EVIDENCE"}),
    );
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("query-1"),
            ev_custom_tool_call("query-call", "exec", &source),
            ev_completed("query-1"),
        ]),
    )
    .await;
    let recovered = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("query-done", "recovered"),
            ev_completed("query-2"),
        ]),
    )
    .await;
    test.submit_turn("Recover the missing evidence.").await?;
    let request = recovered.single_request();
    let texts = custom_tool_output_items(&request, "query-call");
    assert!(text_item(&texts, 1).contains("OUTER-MIDDLE-EVIDENCE"));
    assert_eq!(text_item(&texts, 2), "1");
    Ok(())
}
