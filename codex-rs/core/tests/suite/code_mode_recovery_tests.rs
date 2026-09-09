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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quiet_code_mode_wait_suspends_inference_until_cell_output() -> Result<()> {
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
            ev_response_created("quiet-start"),
            ev_custom_tool_call(
                "quiet-cell",
                "exec",
                r#"
yield_control();
await new Promise(resolve => setTimeout(resolve, 14000));
text("CELL-FINISHED");
"#,
            ),
            ev_completed("quiet-start"),
        ]),
    )
    .await;
    let first_done = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("quiet-start-done", "waiting"),
            ev_completed("quiet-start-2"),
        ]),
    )
    .await;
    test.submit_turn("Start the quiet cell.").await?;
    let request = first_done.single_request();
    let items = custom_tool_output_items(&request, "quiet-cell");
    let cell_id = extract_running_cell_id(text_item(&items, 0));

    let wait_request = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("quiet-wait"),
            responses::ev_function_call(
                "quiet-wait-call",
                "wait",
                &json!({"cell_id": cell_id}).to_string(),
            ),
            ev_completed("quiet-wait"),
        ]),
    )
    .await;
    let completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("quiet-done", "finished"),
            ev_completed("quiet-finish"),
        ]),
    )
    .await;
    tokio::try_join!(test.submit_turn("Wait for the cell result."), async {
        tokio::time::sleep(Duration::from_millis(11_000)).await;
        assert_eq!(wait_request.requests().len(), 1);
        assert_eq!(
            completion.requests().len(),
            0,
            "quiet host waits must remain inside the runtime"
        );
        anyhow::Ok(())
    })?;
    let request = completion.single_request();
    let items = function_tool_output_items(&request, "quiet-wait-call");
    assert_eq!(text_item(&items, 1), "CELL-FINISHED");
    Ok(())
}
