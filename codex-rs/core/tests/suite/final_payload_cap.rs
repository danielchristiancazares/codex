use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_protocol::models::PermissionProfile;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serial_test::serial;
use std::collections::HashMap;
use std::time::Duration;

const OUTPUT_LIMIT: usize = 10_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn high_cardinality_mcp_output_stays_within_configured_cap() -> anyhow::Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "high-cardinality-output";
    let server_name = "rmcp";
    let namespace = format!("mcp__{server_name}");
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                "high_cardinality_output",
                "{}",
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let fixture = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::Stdio {
                        command,
                        args: Vec::new(),
                        env: Some(HashMap::from([(
                            "MCP_TEST_HIGH_CARDINALITY_OUTPUT".to_string(),
                            "1".to_string(),
                        )])),
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    environment_id,
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    omit_tools_from: None,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test mcp servers should accept any configuration");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&fixture.codex, server_name).await?;

    fixture
        .submit_turn_with_permission_profile(
            "call the high-cardinality output tool",
            PermissionProfile::read_only(),
        )
        .await?;

    let output_item = final_mock.single_request().function_call_output(call_id);
    let output = &output_item["output"];
    let items = output
        .as_array()
        .expect("MCP output should remain a content-item array");
    let serialized = serde_json::to_string(output)?;
    let token_count = tiktoken_rs::o200k_base()?.count_ordinary(&serialized);

    assert!(
        token_count <= OUTPUT_LIMIT,
        "{token_count} > {OUTPUT_LIMIT}"
    );
    assert!(items.len() < 12_001);
    assert_eq!(output_item["type"], "function_call_output");
    server.verify().await;
    Ok(())
}
