use anyhow::Result;
use codex_protocol::models::FunctionCallOutputPayload;
use core_test_support::TestTargetOs;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::test_target_os;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn truncated_terminal_evidence_is_recovered_without_another_execution() -> Result<()> {
    let harness =
        TestCodexHarness::with_auto_env_builder(test_codex().with_model("gpt-5.4")).await?;
    let marker = "RECOVER-MIDDLE-EVIDENCE";
    harness
        .write_file(
            "captured-fixture.txt",
            format!(
                "{}{}\n{}",
                "before\n".repeat(10_000),
                marker,
                "after\n".repeat(10_000)
            ),
        )
        .await?;
    let command = match test_target_os() {
        TestTargetOs::Windows => "cmd /c type captured-fixture.txt",
        TestTargetOs::Linux | TestTargetOs::MacOs => "cat captured-fixture.txt",
    };
    let first = mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("capture-1"),
                ev_function_call(
                    "capture-exec",
                    "exec_command",
                    &json!({
                        "cmd": command,
                        "max_output_tokens": 100,
                    })
                    .to_string(),
                ),
                ev_completed("capture-1"),
            ]),
            sse(vec![
                ev_response_created("capture-2"),
                ev_assistant_message("capture-done", "captured"),
                ev_completed("capture-2"),
            ]),
        ],
    )
    .await;
    harness.submit("Read the fixture once.").await?;
    let requests = first.requests();
    assert_eq!(requests.len(), 2);
    let payload: FunctionCallOutputPayload =
        serde_json::from_value(requests[1].function_call_output("capture-exec")["output"].clone())?;
    let preview = payload.body.to_text().expect("text output");
    assert!(!preview.contains(marker));
    let output_id = preview
        .strip_prefix("Captured output ")
        .expect("receipt survives truncation")
        .split('.')
        .next()
        .expect("capture ID");
    uuid::Uuid::parse_str(output_id)?;

    let recovery = mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("recover-1"),
                ev_function_call(
                    "capture-read",
                    "read_output",
                    &json!({
                        "output_id": output_id,
                        "query": "search",
                        "text": marker,
                    })
                    .to_string(),
                ),
                ev_completed("recover-1"),
            ]),
            sse(vec![
                ev_response_created("recover-2"),
                ev_assistant_message("recover-done", "recovered"),
                ev_completed("recover-2"),
            ]),
        ],
    )
    .await;
    harness
        .submit("Recover the middle evidence from the saved output.")
        .await?;
    let recovered_requests = recovery.requests();
    assert_eq!(recovered_requests.len(), 2);
    let payload: FunctionCallOutputPayload = serde_json::from_value(
        recovered_requests[1].function_call_output("capture-read")["output"].clone(),
    )?;
    let recovered = payload.body.to_text().expect("retrieved text");
    assert!(recovered.contains(marker));
    assert!(recovered.len() <= 3_200);
    let executions = recovered_requests[1].body_json()["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["type"] == "function_call" && item["name"] == "exec_command")
        .count();
    assert_eq!(executions, 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quiet_terminal_poll_suspends_inference_until_process_output() -> Result<()> {
    let harness =
        TestCodexHarness::with_auto_env_builder(test_codex().with_model("gpt-5.4")).await?;
    let arguments = match test_target_os() {
        TestTargetOs::Windows => json!({
            "cmd": "Start-Sleep -Seconds 20; [Console]::WriteLine('TERMINAL-FINISHED')",
            "shell": "powershell.exe",
            "login": false,
            "yield_time_ms": 10000,
        }),
        TestTargetOs::Linux | TestTargetOs::MacOs => json!({
            "cmd": "sleep 10; printf TERMINAL-FINISHED",
            "yield_time_ms": 1000,
        }),
    };
    let first = mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("terminal-1"),
                ev_function_call("terminal-start", "exec_command", &arguments.to_string()),
                ev_completed("terminal-1"),
            ]),
            sse(vec![
                ev_assistant_message("terminal-started", "running"),
                ev_completed("terminal-2"),
            ]),
        ],
    )
    .await;
    harness.submit("Start the delayed process.").await?;
    let requests = first.requests();
    let payload: FunctionCallOutputPayload = serde_json::from_value(
        requests[1].function_call_output("terminal-start")["output"].clone(),
    )?;
    let text = payload.body.to_text().unwrap();
    let session_id = text
        .lines()
        .find_map(|line| line.strip_prefix("Process running with session ID "))
        .expect("live terminal")
        .parse::<i32>()?;
    let wait = core_test_support::responses::mount_sse_once(
        harness.server(),
        sse(vec![
            ev_response_created("terminal-poll"),
            ev_function_call(
                "terminal-poll-call",
                "write_stdin",
                &json!({"session_id": session_id}).to_string(),
            ),
            ev_completed("terminal-poll"),
        ]),
    )
    .await;
    let done = core_test_support::responses::mount_sse_once(
        harness.server(),
        sse(vec![
            ev_assistant_message("terminal-finished", "finished"),
            ev_completed("terminal-done"),
        ]),
    )
    .await;
    tokio::try_join!(harness.submit("Wait for its output."), async {
        tokio::time::sleep(Duration::from_millis(6_000)).await;
        assert_eq!(wait.requests().len(), 1);
        assert_eq!(
            done.requests().len(),
            0,
            "empty polling must not resume inference"
        );
        anyhow::Ok(())
    })?;
    let request = done.single_request();
    let payload: FunctionCallOutputPayload = serde_json::from_value(
        request.function_call_output("terminal-poll-call")["output"].clone(),
    )?;
    assert!(
        payload
            .body
            .to_text()
            .unwrap()
            .contains("TERMINAL-FINISHED")
    );
    Ok(())
}
