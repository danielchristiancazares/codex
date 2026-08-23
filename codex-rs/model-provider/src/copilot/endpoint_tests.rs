use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_models_manager::manager::ModelsEndpointClient;
use http::header::ACCEPT;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::super::models_endpoint::CopilotModelsEndpoint;
use super::*;

fn manager_with_credential(credential: CopilotCredential) -> CopilotEndpointManager {
    CopilotEndpointManager {
        state: Mutex::new(EndpointState::default()),
        rejected_generations: StdMutex::new(HashSet::new()),
        credential_loader: Arc::new(move || Ok(Some(credential.clone()))),
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    }
}

fn manager_with_credential_error(error: CredentialLoadError) -> CopilotEndpointManager {
    CopilotEndpointManager {
        state: Mutex::new(EndpointState::default()),
        rejected_generations: StdMutex::new(HashSet::new()),
        credential_loader: Arc::new(move || Err(error.clone())),
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    }
}

fn exchange_manager(server: &MockServer) -> CopilotEndpointManager {
    manager_with_credential(CopilotCredential::GitHubToken {
        token: "github-secret".to_string(),
        token_url: format!("{}/copilot_internal/v2/token", server.uri()),
    })
}

async fn request_models(
    body: serde_json::Value,
) -> codex_protocol::error::Result<(
    Vec<codex_protocol::openai_models::ModelInfo>,
    Option<String>,
)> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("accept", "application/json"))
        .and(header("authorization", "Bearer copilot-secret"))
        .and(header("openai-intent", super::super::identity::INTENT))
        .and(header(
            "editor-version",
            super::super::identity::EDITOR_VERSION,
        ))
        .and(header(
            "editor-plugin-version",
            super::super::identity::EDITOR_PLUGIN_VERSION,
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "catalog-v1")
                .set_body_json(body),
        )
        .expect(1)
        .mount(&server)
        .await;
    let manager = Arc::new(manager_with_credential(CopilotCredential::ApiToken {
        token: "copilot-secret".to_string(),
        api_url: server.uri(),
    }));
    let endpoint = CopilotModelsEndpoint::new(manager);

    ModelsEndpointClient::list_models(
        &endpoint,
        "test-client",
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
}

#[tokio::test]
async fn endpoint_resolution_exchanges_github_token_without_cli_session() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/copilot_internal/v2/token"))
        .and(header("accept", "application/json"))
        .and(header("authorization", "token github-secret"))
        .and(header(
            "user-agent",
            super::super::identity::TOKEN_EXCHANGE_USER_AGENT,
        ))
        .and(header(
            "editor-version",
            super::super::identity::EDITOR_VERSION,
        ))
        .and(header(
            "editor-plugin-version",
            super::super::identity::EDITOR_PLUGIN_VERSION,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "copilot-secret",
            "expires_at": 4_102_444_800_i64,
            "endpoints": {"api": server.uri()}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let manager = exchange_manager(&server);

    let (first, second) = tokio::join!(
        manager.endpoint(),
        manager.endpoint_for_model(ThreadId::new(), "gpt-5.6-sol")
    );
    let first = first.expect("resolve Copilot endpoint");
    let second = second.expect("share Copilot endpoint resolution");

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.base_url, server.uri());
    assert_eq!(first.source, EndpointSource::Direct);
    assert_eq!(first.bound_model.as_deref(), None);
    assert_eq!(
        first
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer copilot-secret")
    );
    assert!(
        first
            .headers
            .get(AUTHORIZATION)
            .is_some_and(HeaderValue::is_sensitive)
    );
    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0]
            .headers
            .get(ACCEPT)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    assert!(!requests[0].headers.contains_key("x-github-api-version"));
}

#[tokio::test]
async fn explicit_direct_credential_exchange_failure_does_not_use_cli() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/copilot_internal/v2/token"))
        .respond_with(ResponseTemplate::new(401).set_body_string("credential rejected"))
        .expect(1)
        .mount(&server)
        .await;
    let manager = exchange_manager(&server);

    let error = manager
        .endpoint_for_model(ThreadId::new(), "gpt-5.6-sol")
        .await
        .expect_err("direct exchange failure must surface");

    assert_eq!(
        error.to_string(),
        "Fatal error: Copilot API token request returned 401 Unauthorized: credential rejected"
    );
}

#[tokio::test]
async fn credential_store_failure_does_not_become_cli_fallback() {
    let manager = manager_with_credential_error(CredentialLoadError::credential_store(
        "credential store unavailable".to_string(),
    ));

    let error = manager
        .endpoint()
        .await
        .expect_err("credential store failure must surface");

    assert_eq!(
        error.to_string(),
        "Fatal error: credential store unavailable"
    );
}

#[tokio::test]
async fn native_copilot_credential_needs_no_session_or_token_exchange() {
    let server = MockServer::start().await;
    let manager = manager_with_credential(CopilotCredential::ApiToken {
        token: "copilot-secret".to_string(),
        api_url: server.uri(),
    });

    let endpoint = manager.endpoint().await.expect("resolve Copilot endpoint");

    assert_eq!(endpoint.base_url, server.uri());
    assert_eq!(endpoint.source, EndpointSource::Direct);
    assert_eq!(
        endpoint
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer copilot-secret")
    );
    assert!(
        server
            .received_requests()
            .await
            .expect("recorded requests")
            .is_empty()
    );
}

#[tokio::test]
async fn models_request_uses_http_path_and_returns_eligible_catalog() {
    let (models, etag) = request_models(serde_json::json!({
        "data": [{
            "id": "gpt-5.6-sol",
            "name": "GPT-5.6 Sol",
            "vendor": "OpenAI",
            "model_picker_enabled": true,
            "supported_endpoints": ["/responses", "ws:/responses"],
            "capabilities": {"type": "chat"}
        }]
    }))
    .await
    .expect("list Copilot models");

    assert_eq!(
        models
            .iter()
            .map(|model| model.slug.as_str())
            .collect::<Vec<_>>(),
        vec!["gpt-5.6-sol"]
    );
    assert_eq!(etag.as_deref(), Some("catalog-v1"));
}

#[tokio::test]
async fn models_request_rejects_empty_raw_catalog() {
    let error = request_models(serde_json::json!({"data": []}))
        .await
        .expect_err("empty raw catalog must fail");

    assert_eq!(
        error.to_string(),
        "Fatal error: Copilot models response contained no model entries"
    );
}

#[tokio::test]
async fn models_request_rejects_catalog_without_websocket_responses() {
    let error = request_models(serde_json::json!({
        "data": [{
            "id": "http-only",
            "model_picker_enabled": true,
            "supported_endpoints": ["/responses"],
            "capabilities": {"type": "chat"}
        }]
    }))
    .await
    .expect_err("ineligible catalog must fail");

    assert_eq!(
        error.to_string(),
        "Fatal error: Copilot models response contained 1 model entries, but none were enabled for Responses-over-WebSocket"
    );
}

#[test]
fn endpoint_rejection_targets_only_the_exact_generation() {
    let manager = CopilotEndpointManager::default();

    manager.reject_generation(7);

    assert!(manager.is_generation_rejected(7));
    assert!(!manager.is_generation_rejected(6));
    assert!(!manager.is_generation_rejected(8));
}

#[test]
fn token_refresh_precedes_expiry() {
    assert_eq!(
        refresh_after(Some(/*expires_at*/ 1_600), /*now*/ 1_000),
        Duration::from_secs(480)
    );
    assert_eq!(
        refresh_after(Some(/*expires_at*/ 1_100), /*now*/ 1_000),
        Duration::from_secs(80)
    );
    assert_eq!(
        refresh_after(/*expires_at*/ None, /*now*/ 1_000),
        DEFAULT_TOKEN_REFRESH
    );
}

#[test]
fn copilot_api_url_requires_https_or_loopback() {
    assert_eq!(
        validate_api_url("https://api.githubcopilot.com/"),
        Ok("https://api.githubcopilot.com".to_string())
    );
    assert_eq!(
        validate_api_url("http://127.0.0.1:8080/"),
        Ok("http://127.0.0.1:8080".to_string())
    );
    assert_eq!(
        validate_api_url("http://api.githubcopilot.com"),
        Err("Copilot API URL must use HTTPS unless it targets loopback".to_string())
    );
}
