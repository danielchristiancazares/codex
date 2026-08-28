//! MCP output-budget coverage runs on native Windows as well as Unix hosts.

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;
use anyhow::Context;
use anyhow::Result;
use codex_history::CodexHarnessMetadata;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_rollout::RolloutItem;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::sync::LazyLock;
use test_case::test_case;
use wiremock::MockServer;

static OUTPUT_TOKENIZER: LazyLock<tiktoken_rs::CoreBPE> =
    LazyLock::new(|| tiktoken_rs::o200k_base().expect("initialize output tokenizer"));

fn assert_output_fits(output: &Value, token_limit: usize) {
    let serialized = serde_json::to_string(output).expect("serialize model output");
    let count = OUTPUT_TOKENIZER.count_ordinary(&serialized);
    assert!(
        count <= token_limit,
        "output uses {count} tokens, limit {token_limit}"
    );
}

fn output_text(output: &Value) -> Result<String> {
    serde_json::from_value::<FunctionCallOutputPayload>(output.clone())?
        .body
        .to_text()
        .context("model-facing MCP output text")
}

async fn call_mcp_echo(
    server: &MockServer,
    builder: TestCodexBuilder,
    output_token_limit: Option<usize>,
    message_bytes: usize,
) -> Result<(TestCodex, Value)> {
    let call_id = "rmcp-output";
    let namespace = "mcp__rmcp";
    responses::mount_sse_once(
        server,
        responses::sse(vec![
            responses::ev_response_created("response-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                namespace,
                "echo",
                &json!({"message": "a".repeat(message_bytes)}).to_string(),
            ),
            responses::ev_completed("response-1"),
        ]),
    )
    .await;
    let response = responses::mount_sse_once(
        server,
        responses::sse(vec![
            responses::ev_assistant_message("message-1", "MCP echo completed."),
            responses::ev_completed("response-2"),
        ]),
    )
    .await;
    let mcp_server = serde_json::from_value(json!({
        "command": remote_aware_stdio_server_bin()?,
        "environment_id": remote_aware_environment_id(),
        "startup_timeout_sec": 10,
        "tools": { "echo": { "output_token_limit": output_token_limit } },
    }))?;
    let mut builder = builder.with_config(move |config| {
        config
            .mcp_servers
            .set(HashMap::from([("rmcp".to_string(), mcp_server)]))
            .expect("set test MCP server");
    });
    let fixture = builder.build_with_auto_env(server).await?;
    wait_for_mcp_server(&fixture.codex, "rmcp").await?;
    fixture.submit_text_turn("call the MCP echo tool").await?;
    let item = response.single_request().function_call_output(call_id);
    assert_output_fits(&item, 10_000);
    let output = item["output"].clone();
    Ok((fixture, output))
}

#[test_case(Some(3_000), 13_000; "tool budget")]
#[test_case(Some(30_000), 70_000; "large configuration within hard ceiling")]
#[test_case(None, 70_000; "model budget within hard ceiling")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_output_limit_preserves_output_that_fits(
    output_token_limit: Option<usize>,
    message_bytes: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = responses::start_mock_server().await;
    let builder = test_codex().with_config(|config| config.tool_output_token_limit = Some(50_000));
    let (_fixture, output) =
        call_mcp_echo(&server, builder, output_token_limit, message_bytes).await?;
    assert!(output_text(&output)?.contains(&"a".repeat(message_bytes)));
    assert_output_fits(&output, output_token_limit.unwrap_or(10_000).min(10_000));
    Ok(())
}

#[test_case(50, 30_000, 50, 150_000; "current global policy")]
#[test_case(50_000, 128, 128, 150_000; "smaller tool budget")]
#[test_case(50_000, 30_000, 10_000, 150_000; "hard ceiling")]
#[test_case(50_000, 30_000, 10_000, 79_650; "envelope reservation")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_output_limit_truncates_oversized_output(
    global_limit: usize,
    tool_limit: usize,
    expected_limit: usize,
    message_bytes: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = responses::start_mock_server().await;
    let builder =
        test_codex().with_config(move |config| config.tool_output_token_limit = Some(global_limit));
    let (_fixture, output) =
        call_mcp_echo(&server, builder, Some(tool_limit), message_bytes).await?;
    assert!(output_text(&output)?.contains("truncated"));
    assert_output_fits(&output, expected_limit);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_output_limit_combines_byte_policy_with_saved_tokens() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = responses::start_mock_server().await;
    let builder = test_codex().with_model_info_override("gpt-5.5", |model| {
        model.truncation_policy = TruncationPolicyConfig::bytes(512);
    });
    let (_fixture, output) =
        call_mcp_echo(&server, builder, Some(64), /*message_bytes*/ 150_000).await?;
    assert!(serde_json::to_string(&output)?.len() <= 512);
    assert_output_fits(&output, 64);
    assert!(output_text(&output)?.contains("truncated"));
    Ok(())
}

#[test_case(50, 100; "global policy restricts hook feedback")]
#[test_case(500, 64; "tool policy restricts hook feedback")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_output_limit_applies_to_hook_feedback(
    global_limit: usize,
    tool_limit: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = responses::start_mock_server().await;
    let builder = test_codex()
        .with_pre_build_hook(|home| {
            let response =
                json!({"continue": false, "stopReason": "hook feedback ".repeat(100)}).to_string();
            let hooks = json!({"hooks": {"PostToolUse": [{
                "matcher": "^mcp__rmcp__echo$",
                "hooks": [{"type": "mcp_tool", "server": "rmcp", "tool": "image_scenario",
                    "input": {"scenario": "text_only", "caption": response}, "timeout": 5}],
            }]}});
            std::fs::write(home.join("hooks.json"), hooks.to_string())
                .expect("write MCP post-tool hook");
        })
        .with_config(move |config| {
            core_test_support::hooks::trust_discovered_hooks(config);
            config.tool_output_token_limit = Some(global_limit);
        });
    let (_fixture, output) =
        call_mcp_echo(&server, builder, Some(tool_limit), /*message_bytes*/ 0).await?;
    let text = output_text(&output)?;
    assert!(text.starts_with("hook feedback "));
    assert!(text.contains("truncated"));
    assert_output_fits(&output, global_limit.min(tool_limit));
    Ok(())
}

#[test_case(None, 50_000; "same model budget")]
#[test_case(Some(128), 50_000; "same tool budget")]
#[test_case(None, 50; "lower current model budget")]
#[test_case(Some(128), 50; "lower model budget restricts saved tool limit")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_output_limit_survives_resume(
    output_token_limit: Option<usize>,
    resume_global_limit: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = responses::start_mock_server().await;
    let builder = test_codex().with_config(|config| config.tool_output_token_limit = Some(50_000));
    let (fixture, output) = call_mcp_echo(
        &server,
        builder,
        output_token_limit,
        /*message_bytes*/ 150_000,
    )
    .await?;
    fixture.codex.ensure_rollout_materialized().await;
    fixture.codex.flush_rollout().await?;
    let saved = fixture
        .codex
        .load_history(/*include_archived*/ false)
        .await?;
    let envelope = saved.items.iter().find_map(|item| match item {
        RolloutItem::ResponseItem(envelope) if matches!(&envelope.item,
            ResponseItem::FunctionCallOutput {call_id: Some(call_id), ..} if call_id == "rmcp-output"
        ) => Some(envelope),
        _ => None,
    }).context("saved MCP output")?;
    assert_eq!(
        envelope.metadata,
        Some(CodexHarnessMetadata {
            fallback_token_limit_override: Some(output_token_limit.unwrap_or(50_000).min(10_000)),
            ..Default::default()
        })
    );
    let resumed_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("message-2", "resumed"),
            responses::ev_completed("response-3"),
        ]),
    )
    .await;
    let mut resume_builder = test_codex()
        .with_config(move |config| config.tool_output_token_limit = Some(resume_global_limit));
    let resumed = resume_builder.restart(&server, &fixture).await?;
    resumed.submit_turn("continue").await?;
    let resumed_output = resumed_response
        .single_request()
        .function_call_output("rmcp-output")["output"]
        .clone();
    assert_output_fits(
        &resumed_output,
        output_token_limit
            .unwrap_or(10_000)
            .min(10_000)
            .min(resume_global_limit),
    );
    if resume_global_limit == 50_000 {
        assert_eq!(resumed_output, output);
    } else {
        assert_ne!(resumed_output, output);
    }
    Ok(())
}
