use std::collections::VecDeque;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_models_manager::manager::ModelsEndpointClient;
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

const TEST_MACHINE_ID: &str = "4f8c2f5df054b1e465c8f9d9af3b391a4718b02ad7c3d0f8e83d4f6978de1451";

fn credential(token: &str, base_url: &str) -> CopilotCredential {
    CopilotCredential {
        token: token.to_string(),
        base_url: base_url.to_string(),
        machine_id: Some(TEST_MACHINE_ID.to_string()),
        source: CopilotCredentialSource::StoredOAuth,
    }
}

fn manager_with_loader(
    loader: impl Fn() -> Result<CopilotCredential, CredentialLoadError> + Send + Sync + 'static,
) -> CopilotEndpointManager {
    CopilotEndpointManager {
        state: Mutex::new(EndpointState::default()),
        current_generation: AtomicU64::new(0),
        rejected_generations: StdMutex::new(HashSet::new()),
        credential_loader: Arc::new(loader),
    }
}

fn manager_with_credential(credential: CopilotCredential) -> CopilotEndpointManager {
    manager_with_loader(move || Ok(credential.clone()))
}

fn manager_with_credential_error(error: CredentialLoadError) -> CopilotEndpointManager {
    manager_with_loader(move || Err(error.clone()))
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
            "x-client-application",
            super::super::identity::CLIENT_APPLICATION,
        ))
        .and(header(
            "user-agent",
            super::super::identity::USER_AGENT_VALUE,
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "catalog-v1")
                .set_body_json(body),
        )
        .expect(1)
        .mount(&server)
        .await;
    let manager = Arc::new(manager_with_credential(credential(
        "copilot-secret",
        &server.uri(),
    )));
    let endpoint = CopilotModelsEndpoint::new(manager);

    ModelsEndpointClient::list_models(
        &endpoint,
        "test-client",
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
}

#[tokio::test]
async fn concurrent_endpoint_resolution_loads_one_direct_credential() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loader_loads = Arc::clone(&loads);
    let manager = manager_with_loader(move || {
        loader_loads.fetch_add(1, Ordering::Relaxed);
        Ok(credential("github-secret", "https://api.githubcopilot.com"))
    });

    let (first, second) = tokio::join!(manager.endpoint(), manager.endpoint());
    let first = first.expect("resolve Copilot endpoint");
    let second = second.expect("share Copilot endpoint resolution");
    let third = manager.endpoint().await.expect("reuse warm endpoint");

    assert!(Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first, &third));
    assert_eq!(loads.load(Ordering::Relaxed), 1);
    assert_eq!(first.base_url, "https://api.githubcopilot.com");
    assert_eq!(first.machine_id.as_deref(), Some(TEST_MACHINE_ID));
    assert_eq!(
        first
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer github-secret")
    );
    assert!(
        first
            .headers
            .get(AUTHORIZATION)
            .is_some_and(HeaderValue::is_sensitive)
    );
    assert!(!format!("{manager:?}").contains("github-secret"));
}

#[tokio::test]
async fn credential_store_failure_surfaces() {
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
async fn missing_native_credential_reports_setup_guidance() {
    let manager = manager_with_credential_error(CredentialLoadError::missing());

    let error = manager
        .endpoint()
        .await
        .expect_err("missing native credential must surface");

    assert_eq!(
        error.to_string(),
        "Fatal error: GitHub Copilot native authentication requires \
         `GITHUB_COPILOT_API_TOKEN`, `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, \
         `GITHUB_TOKEN`, or `codex login --provider copilot`"
    );
}

#[tokio::test]
async fn unchanged_rejected_credential_is_terminal_and_single_flight() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loader_loads = Arc::clone(&loads);
    let manager = manager_with_loader(move || {
        loader_loads.fetch_add(1, Ordering::Relaxed);
        Ok(credential(
            "rejected-secret",
            "https://api.githubcopilot.com",
        ))
    });
    let endpoint = manager.endpoint().await.expect("initial endpoint");
    manager.reject_generation(endpoint.generation);

    let (first, second) = tokio::join!(manager.endpoint(), manager.endpoint());
    let first = first.expect_err("unchanged credential must fail");
    let second = second.expect_err("concurrent unchanged credential must fail");

    assert_eq!(first.to_string(), second.to_string());
    assert_eq!(
        first.to_string(),
        "Fatal error: GitHub Copilot Substrate rejected the current credential; run \
         `codex login --provider copilot --force` before retrying"
    );
    assert_eq!(loads.load(Ordering::Relaxed), 2);
    assert!(!first.to_string().contains("rejected-secret"));
}

#[tokio::test]
async fn changed_credential_recovers_and_delayed_rejection_is_harmless() {
    let current = Arc::new(StdMutex::new(credential(
        "old-secret",
        "https://api.githubcopilot.com",
    )));
    let loader_current = Arc::clone(&current);
    let manager = manager_with_loader(move || {
        Ok(loader_current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    });
    let first = manager.endpoint().await.expect("initial endpoint");
    manager.reject_generation(first.generation);
    *current
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        credential("new-secret", "https://api.githubcopilot.com");

    let second = manager.endpoint().await.expect("changed credential");
    manager.reject_generation(first.generation);
    let current = manager
        .endpoint()
        .await
        .expect("current endpoint remains valid");

    assert_eq!(second.generation, first.generation + 1);
    assert!(Arc::ptr_eq(&second, &current));
    assert!(!manager.is_generation_rejected(first.generation));
    assert!(!manager.is_generation_rejected(second.generation));
    assert_eq!(
        second
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer new-secret")
    );
}

#[tokio::test]
async fn models_request_retries_once_after_credential_changes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("authorization", "Bearer old-secret"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("authorization", "Bearer new-secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "id": "gpt-5.6-sol",
                "model_picker_enabled": true,
                "supported_endpoints": ["ws:/responses"]
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let credentials = Arc::new(StdMutex::new(VecDeque::from([
        credential("old-secret", &server.uri()),
        credential("new-secret", &server.uri()),
    ])));
    let loader_credentials = Arc::clone(&credentials);
    let manager = Arc::new(manager_with_loader(move || {
        loader_credentials
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .ok_or_else(CredentialLoadError::missing)
    }));
    let endpoint = CopilotModelsEndpoint::new(manager);

    let (models, etag) = ModelsEndpointClient::list_models(
        &endpoint,
        "test-client",
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
    .expect("changed credential retries model discovery");

    assert_eq!(models.len(), 1);
    assert_eq!(models[0].slug, "gpt-5.6-sol");
    assert_eq!(etag, None);
}

#[tokio::test]
async fn models_request_does_not_retry_an_unchanged_credential() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("authorization", "Bearer rejected-secret"))
        .respond_with(ResponseTemplate::new(401).set_body_string("rejected-secret"))
        .expect(1)
        .mount(&server)
        .await;
    let manager = Arc::new(manager_with_credential(credential(
        "rejected-secret",
        &server.uri(),
    )));
    let endpoint = CopilotModelsEndpoint::new(manager);

    let error = ModelsEndpointClient::list_models(
        &endpoint,
        "test-client",
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
    .expect_err("unchanged rejected credential must fail");

    assert_eq!(
        error.to_string(),
        "Fatal error: GitHub Copilot Substrate rejected the current credential; run \
         `codex login --provider copilot --force` before retrying"
    );
    assert!(!error.to_string().contains("rejected-secret"));
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
fn copilot_api_url_requires_a_safe_origin() {
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
    assert_eq!(
        validate_api_url("https://user:secret@example.com"),
        Err("configured Copilot API URL is invalid".to_string())
    );
    assert_eq!(
        validate_api_url("https://example.com?redirect=other"),
        Err("configured Copilot API URL is invalid".to_string())
    );
}
