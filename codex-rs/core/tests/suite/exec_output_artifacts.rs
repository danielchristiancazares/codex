use anyhow::Context;
use anyhow::Result;
use codex_exec_output_artifacts::ArtifactStore;
use codex_exec_output_artifacts::ArtifactStoreConfig;
use codex_features::Feature;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

fn artifacts_from_exec_output(output: &str) -> Result<Option<Value>> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("Artifacts: "))
        .map(serde_json::from_str)
        .transpose()
        .context("exec-output artifact descriptors should be valid JSON")
}

fn function_output(mock: &ResponseMock, call_id: &str) -> Result<String> {
    mock.function_call_output_text(call_id)
        .with_context(|| format!("missing function-call output for {call_id}"))
}

fn artifact_ref(artifacts: &Value, stream: &str) -> Result<String> {
    artifacts
        .get(stream)
        .and_then(|descriptor| descriptor.get("artifact_ref"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing {stream} artifact reference"))
}

fn environment_id(artifacts: &Value, stream: &str) -> Result<String> {
    artifacts
        .get(stream)
        .and_then(|descriptor| descriptor.get("environment_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing {stream} artifact environment"))
}

async fn build_artifact_test(server: &wiremock::MockServer) -> Result<TestCodex> {
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow unified exec");
        config
            .features
            .enable(Feature::ExecOutputArtifacts)
            .expect("test config should allow exec-output artifacts");
    });
    builder.build_with_auto_env(server).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completed_exec_output_can_be_queried_with_bounded_receipts() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = build_artifact_test(&server).await?;

    let exec_call_id = "artifact-exec";
    let exec_args = json!({
        "cmd": "echo EXEC-OUTPUT-ARTIFACT-MARKER",
        "yield_time_ms": 5_000,
    });
    let exec_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("exec-response"),
                ev_function_call(
                    exec_call_id,
                    "exec_command",
                    &serde_json::to_string(&exec_args)?,
                ),
                ev_completed("exec-response"),
            ]),
            sse(vec![
                ev_assistant_message("exec-done", "captured"),
                ev_completed("exec-final"),
            ]),
        ],
    )
    .await;
    test.submit_turn("capture command output").await?;

    let exec_output = function_output(&exec_mock, exec_call_id)?;
    let artifacts = artifacts_from_exec_output(&exec_output)?
        .context("completed exec output should include artifact descriptors")?;
    assert_eq!(artifacts["stdout"]["state"], "complete");
    assert_eq!(artifacts["stdout"]["capture"], "complete");
    let artifact_ref = artifact_ref(&artifacts, "stdout")?;
    let environment_id = environment_id(&artifacts, "stdout")?;
    assert_eq!(
        environment_id,
        test.executor_environment().selection().environment_id
    );
    server.reset().await;

    let first_query_id = "artifact-query-first";
    let repeated_query_id = "artifact-query-repeated";
    let forced_query_id = "artifact-query-forced";
    let unavailable_query_id = "artifact-query-unavailable";
    let query_args = json!({
        "artifact_ref": artifact_ref,
        "environment_id": environment_id,
        "view": "head",
        "max_output_bytes": 256,
    });
    let forced_query_args = json!({
        "artifact_ref": query_args["artifact_ref"],
        "environment_id": query_args["environment_id"],
        "view": "head",
        "max_output_bytes": 256,
        "include_data": true,
    });
    let unavailable_query_args = json!({
        "artifact_ref": "exec-output-artifact://v1/00000000000000000000000000000000",
        "environment_id": query_args["environment_id"],
        "view": "metadata",
    });
    let query_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("query-response-1"),
                ev_function_call(
                    first_query_id,
                    "exec_output_query",
                    &serde_json::to_string(&query_args)?,
                ),
                ev_completed("query-response-1"),
            ]),
            sse(vec![
                ev_response_created("query-response-2"),
                ev_function_call(
                    repeated_query_id,
                    "exec_output_query",
                    &serde_json::to_string(&query_args)?,
                ),
                ev_completed("query-response-2"),
            ]),
            sse(vec![
                ev_response_created("query-response-3"),
                ev_function_call(
                    forced_query_id,
                    "exec_output_query",
                    &serde_json::to_string(&forced_query_args)?,
                ),
                ev_completed("query-response-3"),
            ]),
            sse(vec![
                ev_response_created("query-response-4"),
                ev_function_call(
                    unavailable_query_id,
                    "exec_output_query",
                    &serde_json::to_string(&unavailable_query_args)?,
                ),
                ev_completed("query-response-4"),
            ]),
            sse(vec![
                ev_assistant_message("query-done", "read"),
                ev_completed("query-final"),
            ]),
        ],
    )
    .await;
    test.submit_turn("read the retained output").await?;

    let first: Value = serde_json::from_str(&function_output(&query_mock, first_query_id)?)?;
    let repeated: Value = serde_json::from_str(&function_output(&query_mock, repeated_query_id)?)?;
    let forced: Value = serde_json::from_str(&function_output(&query_mock, forced_query_id)?)?;
    let unavailable = function_output(&query_mock, unavailable_query_id)?;
    assert!(
        first["data"]["text"]
            .as_str()
            .is_some_and(|text| text.contains("EXEC-OUTPUT-ARTIFACT-MARKER"))
    );
    assert_eq!(first["repeated_slice"], false);
    assert_eq!(repeated["data"], Value::Null);
    assert_eq!(repeated["repeated_slice"], true);
    assert_eq!(forced["data"], first["data"]);
    assert_eq!(forced["repeated_slice"], false);
    assert_eq!(
        unavailable,
        "exec-output artifact is unavailable in the current thread and workspace"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn background_exec_transitions_artifacts_from_pending_to_complete() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = build_artifact_test(&server).await?;

    let start_call_id = "artifact-background-start";
    let poll_call_id = "artifact-background-poll";
    let command = if core_test_support::test_target_os() == core_test_support::TestTargetOs::Windows
    {
        "Write-Output ARTIFACT-START; $null = Read-Host; Write-Output ARTIFACT-FINISH"
    } else {
        "echo ARTIFACT-START; read line; echo ARTIFACT-FINISH"
    };
    let start_args = json!({
        "cmd": command,
        "tty": true,
        "yield_time_ms": 10,
    });
    let poll_args = json!({
        "session_id": 1000,
        "chars": "continue\n",
        "yield_time_ms": 30_000,
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("background-response-1"),
                ev_function_call(
                    start_call_id,
                    "exec_command",
                    &serde_json::to_string(&start_args)?,
                ),
                ev_completed("background-response-1"),
            ]),
            sse(vec![
                ev_response_created("background-response-2"),
                ev_function_call(
                    poll_call_id,
                    "write_stdin",
                    &serde_json::to_string(&poll_args)?,
                ),
                ev_completed("background-response-2"),
            ]),
            sse(vec![
                ev_assistant_message("background-done", "complete"),
                ev_completed("background-final"),
            ]),
        ],
    )
    .await;
    test.submit_turn("run a background command").await?;

    let pending_output = function_output(&mock, start_call_id)?;
    let pending = artifacts_from_exec_output(&pending_output)?
        .context("running command should include pending artifact descriptors")?;
    let complete_output = function_output(&mock, poll_call_id)?;
    let complete = artifacts_from_exec_output(&complete_output)?.with_context(|| {
        format!("completed poll should include finalized artifact descriptors: {complete_output:?}")
    })?;
    assert_eq!(pending["stdout"]["state"], "pending");
    assert_eq!(complete["stdout"]["state"], "complete");
    assert_eq!(
        pending["stdout"]["artifact_ref"],
        complete["stdout"]["artifact_ref"]
    );
    assert!(complete_output.contains("ARTIFACT-FINISH"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn artifact_quota_failure_preserves_command_result() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = build_artifact_test(&server).await?;
    let store = ArtifactStore::open(
        test.codex_home_path().join("quota-test-artifacts"),
        test.codex.thread_extension_data().level_id().to_string(),
        ArtifactStoreConfig {
            thread_bytes_cap: 0,
            ..ArtifactStoreConfig::default()
        },
    )?;
    test.codex.thread_extension_data().insert(store);

    let call_id = "artifact-quota";
    let args = json!({
        "cmd": "echo ARTIFACT-QUOTA-PRESERVED",
        "yield_time_ms": 5_000,
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("quota-response"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
                ev_completed("quota-response"),
            ]),
            sse(vec![
                ev_assistant_message("quota-done", "complete"),
                ev_completed("quota-final"),
            ]),
        ],
    )
    .await;
    test.submit_turn("run despite artifact quota").await?;

    let output = function_output(&mock, call_id)?;
    assert!(output.contains("ARTIFACT-QUOTA-PRESERVED"));
    assert!(output.contains("Process exited with code 0"));
    assert_eq!(artifacts_from_exec_output(&output)?, None);
    Ok(())
}
