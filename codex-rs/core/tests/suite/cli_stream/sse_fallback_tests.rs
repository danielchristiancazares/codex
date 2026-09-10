use super::cli_sse_response;
use super::run_cli_command;
use anyhow::Result;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::process::Command;
use tempfile::TempDir;
use test_case::test_case;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

enum ResponsesTransport {
    HttpOnly,
    UpgradeRequired,
}

#[test_case(ResponsesTransport::HttpOnly; "http_only_provider")]
#[test_case(ResponsesTransport::UpgradeRequired; "websocket_upgrade_required")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_cli_streams_over_sse(transport: ResponsesTransport) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (supports_websockets, expected_methods) = match transport {
        ResponsesTransport::HttpOnly => (false, vec!["POST"]),
        ResponsesTransport::UpgradeRequired => {
            Mock::given(method("GET"))
                .and(path("/v1/responses"))
                .respond_with(ResponseTemplate::new(/*s*/ 426))
                .mount(&server)
                .await;
            (true, vec!["GET", "POST"])
        }
    };
    let response_mock = responses::mount_sse_once(&server, cli_sse_response()).await;
    let home = TempDir::new()?;
    let workspace = TempDir::new()?;
    let prompt = "Reply with the fixture greeting.";
    let provider_override = format!(
        "model_providers.mock={{ name = \"mock\", base_url = \"{}/v1\", \
         env_key = \"OPENAI_API_KEY\", wire_api = \"responses\", \
         supports_websockets = {supports_websockets}, request_max_retries = 0, \
         stream_max_retries = 2 }}",
        server.uri()
    );

    // The subprocess uses production initialization, independently of the test harness.
    let mut command = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("--model")
        .arg("gpt-5.5")
        .arg("-c")
        .arg(provider_override)
        .arg("-c")
        .arg("model_provider=\"mock\"")
        .arg("-C")
        .arg(workspace.path())
        .arg(prompt)
        .env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env_remove(codex_login::CODEX_API_KEY_ENV_VAR)
        .env_remove(codex_login::CODEX_ACCESS_TOKEN_ENV_VAR);
    let output = run_cli_command(&mut command)?;
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "fixture hello"
    );

    let requests = server.received_requests().await.unwrap_or_default();
    let methods: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == "/v1/responses")
        .map(|request| request.method.as_str())
        .collect();
    assert_eq!(methods, expected_methods);
    assert_eq!(
        response_mock
            .single_request()
            .message_input_texts("user")
            .last()
            .map(String::as_str),
        Some(prompt)
    );
    Ok(())
}
