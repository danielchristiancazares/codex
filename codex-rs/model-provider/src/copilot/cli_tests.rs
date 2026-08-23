use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

use super::super::endpoint::CopilotEndpointManager;
use super::*;

#[test]
fn endpoint_rejection_targets_only_the_exact_generation() {
    let manager = CopilotEndpointManager::default();

    manager.reject_generation(7);

    assert!(manager.is_generation_rejected(7));
    assert!(!manager.is_generation_rejected(6));
    assert!(!manager.is_generation_rejected(8));
}

#[tokio::test]
async fn session_create_uses_the_codex_thread_id() {
    let thread_id = ThreadId::new();
    let expected_session_id = thread_id.to_string();
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let (server_reader, mut server_writer) = tokio::io::split(server_stream);
    let server = tokio::spawn(async move {
        let mut server_reader = BufReader::new(server_reader);
        assert_eq!(
            read_json_rpc_frame(&mut server_reader).await,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "session.create",
                "params": {
                    "sessionId": expected_session_id,
                    "model": "gpt-5.6-sol",
                    "clientName": super::super::identity::CLIENT_APPLICATION,
                    "capi": {"enableWebSocketResponses": true},
                    "requestPermission": false,
                    "requestUserInput": false,
                    "requestElicitation": false,
                    "requestExitPlanMode": false,
                    "requestAutoModeSwitch": false,
                    "hooks": false,
                    "includeSubAgentStreamingEvents": true,
                    "envValueMode": "direct"
                }
            })
        );
        write_json_rpc_frame(
            &mut server_writer,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {"sessionId": expected_session_id}
            }),
        )
        .await;

        assert_eq!(
            read_json_rpc_frame(&mut server_reader).await,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "session.provider.getEndpoint",
                "params": {
                    "sessionId": expected_session_id,
                    "modelId": "gpt-5.6-sol"
                }
            })
        );
        write_json_rpc_frame(
            &mut server_writer,
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "type": "openai",
                    "wireApi": "responses",
                    "transport": "websockets",
                    "baseUrl": "https://api.githubcopilot.com",
                    "headers": {}
                }
            }),
        )
        .await;

        assert_eq!(
            read_json_rpc_frame(&mut server_reader).await,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "session.delete",
                "params": {"sessionId": expected_session_id}
            })
        );
        write_json_rpc_frame(
            &mut server_writer,
            &json!({"jsonrpc": "2.0", "id": 3, "result": {}}),
        )
        .await;
    });

    let mut rpc = JsonRpcClient::new(client_reader, client_writer);
    let endpoint = resolve_model_endpoint(
        &mut rpc,
        ProviderSessionIdentity::CodexThread(thread_id),
        "gpt-5.6-sol",
        Some("gpt-5.6-sol"),
    )
    .await
    .expect("resolve endpoint");

    assert_eq!(endpoint.base_url, "https://api.githubcopilot.com");
    assert_eq!(endpoint.source, EndpointSource::Cli);
    server.await.expect("mock Copilot CLI");
}

#[test]
fn builds_sensitive_endpoint_headers_without_exposing_values_in_debug() {
    let endpoint = ProviderEndpoint {
        kind: "openai".to_string(),
        wire_api: Some("responses".to_string()),
        transport: Some("websockets".to_string()),
        base_url: "https://api.githubcopilot.com/".to_string(),
        api_key: Some("api-secret".to_string()),
        headers: HashMap::from([(
            "Copilot-Integration-Id".to_string(),
            "copilot-developer-cli".to_string(),
        )]),
        session_token: Some(ProviderSessionToken {
            token: "session-secret".to_string(),
            header: "X-Copilot-Session-Token".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            expires_at: Some("2030-01-01T00:00:00Z".to_string()),
        }),
    };

    let endpoint_debug = format!("{endpoint:?}");
    let resolved = build_endpoint(endpoint, Some("gpt-5.6-sol")).expect("valid endpoint");

    assert_eq!(resolved.base_url, "https://api.githubcopilot.com");
    assert_eq!(resolved.bound_model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(resolved.source, EndpointSource::Cli);
    assert_eq!(
        resolved
            .headers
            .get("copilot-integration-id")
            .and_then(|value| value.to_str().ok()),
        Some("copilot-developer-cli")
    );
    assert!(
        resolved
            .headers
            .get(AUTHORIZATION)
            .is_some_and(HeaderValue::is_sensitive)
    );
    assert!(
        resolved
            .headers
            .get("x-copilot-session-token")
            .is_some_and(HeaderValue::is_sensitive)
    );
    assert!(!endpoint_debug.contains("api-secret"));
    assert!(!endpoint_debug.contains("session-secret"));
}

#[test]
fn orders_preferred_model_first_for_catalog_broker() {
    let models = vec![
        RpcModel {
            id: "gpt-5.4".to_string(),
        },
        RpcModel {
            id: "gpt-5.6-sol".to_string(),
        },
    ];

    assert_eq!(
        endpoint_model_candidates(&models, /*requested_model*/ None),
        Ok(vec!["gpt-5.6-sol", "gpt-5.4"])
    );
}

#[test]
fn catalog_broker_can_bootstrap_from_non_gpt_model() {
    let models = vec![RpcModel {
        id: "claude-sonnet-4.5".to_string(),
    }];

    assert_eq!(
        endpoint_model_candidates(&models, /*requested_model*/ None),
        Ok(vec!["claude-sonnet-4.5"])
    );
}

#[test]
fn requested_inference_model_is_selected_exactly() {
    let models = vec![
        RpcModel {
            id: "gpt-5.6-sol".to_string(),
        },
        RpcModel {
            id: "claude-sonnet-4.5".to_string(),
        },
    ];

    assert_eq!(
        endpoint_model_candidates(&models, Some("claude-sonnet-4.5")),
        Ok(vec!["claude-sonnet-4.5"])
    );
    assert_eq!(
        endpoint_model_candidates(&models, Some("unavailable-model")),
        Err("Copilot CLI model `unavailable-model` is unavailable for this account".to_string())
    );
}

#[test]
fn rejects_non_responses_endpoint() {
    let endpoint = ProviderEndpoint {
        kind: "openai".to_string(),
        wire_api: Some("completions".to_string()),
        transport: None,
        base_url: "https://api.githubcopilot.com".to_string(),
        api_key: None,
        headers: HashMap::new(),
        session_token: None,
    };

    assert_eq!(
        build_endpoint(endpoint, /*requested_model*/ None)
            .expect_err("completions must be rejected"),
        "Copilot CLI selected unsupported wire API `completions`"
    );
}

#[test]
fn rejects_explicit_http_transport() {
    let endpoint = ProviderEndpoint {
        kind: "openai".to_string(),
        wire_api: Some("responses".to_string()),
        transport: Some("http".to_string()),
        base_url: "https://api.githubcopilot.com".to_string(),
        api_key: None,
        headers: HashMap::new(),
        session_token: None,
    };

    assert_eq!(
        build_endpoint(endpoint, /*requested_model*/ None)
            .expect_err("HTTP transport must be rejected"),
        "Copilot CLI selected unsupported transport `http`"
    );
}

#[test]
fn rejects_session_token_bound_to_another_model() {
    let endpoint = ProviderEndpoint {
        kind: "openai".to_string(),
        wire_api: Some("responses".to_string()),
        transport: Some("websockets".to_string()),
        base_url: "https://api.githubcopilot.com".to_string(),
        api_key: None,
        headers: HashMap::new(),
        session_token: Some(ProviderSessionToken {
            token: "session-secret".to_string(),
            header: "X-Copilot-Session-Token".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            expires_at: None,
        }),
    };

    assert_eq!(
        build_endpoint(endpoint, Some("claude-sonnet-4.5"))
            .expect_err("model-bound token must match the request"),
        "Copilot CLI bound model `gpt-5.6-sol` does not match requested model `claude-sonnet-4.5`"
    );
}

async fn read_json_rpc_frame<R>(reader: &mut R) -> Value
where
    R: AsyncBufRead + Unpin,
{
    let mut content_length = None;
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .expect("read JSON-RPC header");
        if matches!(line.as_str(), "\r\n" | "\n") {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = Some(value.trim().parse::<usize>().expect("parse content length"));
        }
    }
    let mut payload = vec![0; content_length.expect("content length")];
    reader
        .read_exact(&mut payload)
        .await
        .expect("read JSON-RPC payload");
    serde_json::from_slice(&payload).expect("decode JSON-RPC payload")
}

async fn write_json_rpc_frame<W>(writer: &mut W, message: &Value)
where
    W: AsyncWrite + Unpin,
{
    let payload = serde_json::to_vec(message).expect("encode JSON-RPC payload");
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())
        .await
        .expect("write JSON-RPC header");
    writer
        .write_all(&payload)
        .await
        .expect("write JSON-RPC payload");
    writer.flush().await.expect("flush JSON-RPC payload");
}
