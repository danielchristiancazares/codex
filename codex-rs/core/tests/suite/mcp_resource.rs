use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use codex_config::types::McpServerAuth;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_features::Feature;
use codex_protocol::openai_models::InputModality;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::MockServer;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

async fn resource_fixture(
    server: &MockServer,
    env: Option<HashMap<String, String>>,
) -> Result<TestCodex> {
    let server_name = "rmcp";
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let fixture = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.supports_search_tool = false;
            model.input_modalities = vec![InputModality::Text, InputModality::Image];
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Sqlite)
                .expect("test config should allow SQLite");
            config.memories.disable_on_external_context = true;
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command,
                        args: Vec::new(),
                        env,
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    auth: McpServerAuth::default(),
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
                .expect("test MCP server configuration should be valid");
        })
        .build_with_auto_env(server)
        .await?;
    wait_for_mcp_server(&fixture.codex, server_name).await?;
    Ok(fixture)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cf_046_mcp_resource_marks_thread_memory_mode_polluted() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let fixture = resource_fixture(&server, /*env*/ None).await?;
    let call_id = "read-resource-for-memory-mode";
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(
                    call_id,
                    "read_mcp_resource",
                    &json!({
                        "server": "rmcp",
                        "uri": "memo://codex/example-note",
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let state_db = fixture.codex.state_db().expect("SQLite enabled");
    fixture.submit_turn("Read the test MCP resource").await?;

    assert_eq!(
        state_db
            .get_thread_memory_mode(fixture.session_configured.thread_id)
            .await?,
        Some("polluted".to_string())
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_server_resource_listings_emit_server_once() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let server_name = "rmcp";
    let resources_call_id = "list-resources";
    let templates_call_id = "list-resource-templates";
    let fixture = resource_fixture(&server, /*env*/ None).await?;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(
                    resources_call_id,
                    "list_mcp_resources",
                    &json!({"server": server_name}).to_string(),
                ),
                responses::ev_function_call(
                    templates_call_id,
                    "list_mcp_resource_templates",
                    &json!({"server": server_name}).to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    fixture.submit_turn("List the test MCP resources").await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let follow_up = requests
        .last()
        .expect("resource listings should issue a follow-up request");
    let resources = serde_json::from_str::<serde_json::Value>(
        &follow_up
            .function_call_output_text(resources_call_id)
            .expect("resource listing output"),
    )?;
    assert_eq!(
        resources,
        json!({
            "server": server_name,
            "resources": [{
                "uri": "memo://codex/example-note",
                "name": "example-note",
                "title": "Example Note",
                "description": "A sample MCP resource exposed for integration tests.",
                "mimeType": "text/plain",
            }],
        })
    );
    let templates = serde_json::from_str::<serde_json::Value>(
        &follow_up
            .function_call_output_text(templates_call_id)
            .expect("resource template listing output"),
    )?;
    assert_eq!(
        templates,
        json!({
            "server": server_name,
            "resourceTemplates": [{
                "uriTemplate": "memo://codex/{slug}",
                "name": "codex-memo",
                "title": "Codex Memo",
                "description": "Template for memo://codex/{slug} resources used in tests.",
                "mimeType": "text/plain",
            }],
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_mcp_image_resource_emits_typed_image_content() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let server_name = "rmcp";
    let call_id = "read-image-resource";
    let fixture = resource_fixture(
        &server,
        Some(HashMap::from([(
            "MCP_TEST_RESOURCE_CONTENT_KIND".to_string(),
            "image".to_string(),
        )])),
    )
    .await?;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(
                    call_id,
                    "read_mcp_resource",
                    &json!({
                        "server": server_name,
                        "uri": "memo://codex/example-note",
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    fixture
        .submit_turn("Read the test MCP image resource")
        .await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let output = requests
        .last()
        .expect("resource read should issue a follow-up request")
        .function_call_output(call_id);
    let content = output["output"]
        .as_array()
        .expect("resource image output should use typed content items");
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "input_text");
    assert_eq!(
        content[0]["text"],
        "MCP resource `memo://codex/example-note` from `rmcp` (image/png)"
    );
    assert_eq!(content[1]["type"], "input_image");
    assert!(
        content[1]["image_url"]
            .as_str()
            .is_some_and(|image_url| image_url.starts_with("data:image/png;base64,"))
    );
    assert!(
        content
            .iter()
            .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
            .all(|text| !text.contains("iVBOR"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn embedded_mcp_image_resource_emits_typed_image_content() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let call_id = "embedded-image-resource";
    let fixture = resource_fixture(&server, /*env*/ None).await?;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "mcp__rmcp",
                    "image_scenario",
                    r#"{"scenario":"embedded_image_resource"}"#,
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    fixture
        .submit_turn("Return the embedded MCP image resource")
        .await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let output = requests
        .last()
        .expect("embedded resource call should issue a follow-up request")
        .function_call_output(call_id);
    let content = output["output"]
        .as_array()
        .expect("embedded resource output should use typed content items");
    assert_eq!(content.len(), 3);
    assert_eq!(content[0]["type"], "input_text");
    assert!(
        content[0]["text"]
            .as_str()
            .is_some_and(|text| text.starts_with("Wall time:"))
    );
    assert_eq!(
        content[1],
        json!({
            "type": "input_text",
            "text": "MCP embedded resource `image://embedded-example` (image/png)",
        })
    );
    assert_eq!(content[2]["type"], "input_image");
    assert!(
        content[2]["image_url"]
            .as_str()
            .is_some_and(|image_url| image_url.starts_with("data:image/png;base64,"))
    );
    assert!(
        content
            .iter()
            .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
            .all(|text| !text.contains("iVBOR"))
    );

    Ok(())
}
