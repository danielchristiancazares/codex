use std::collections::VecDeque;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::ConnectionProvider;
use codex_login::NonEmptyString;
use codex_models_manager::manager::ModelsEndpointClient;
use codex_models_manager::manager::ModelsEndpointResponse;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::OpenAiModelsManager;
use codex_models_manager::manager::RefreshStrategy;
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

#[test]
fn endpoint_managers_share_auth_state_only_within_the_same_runtime() {
    let first_auth = AuthManager::from_auth_for_testing(codex_login::CodexAuth::from_api_key(
        "first-openai-token",
    ));
    let second_auth = AuthManager::from_auth_for_testing(codex_login::CodexAuth::from_api_key(
        "second-openai-token",
    ));
    let first = shared_endpoint_manager(Some(first_auth.clone()));
    let same_runtime = shared_endpoint_manager(Some(first_auth));
    let other_runtime = shared_endpoint_manager(Some(second_auth));

    assert!(Arc::ptr_eq(&first, &same_runtime));
    assert!(!Arc::ptr_eq(&first, &other_runtime));
}

#[tokio::test]
async fn endpoint_manager_changes_with_the_selected_saved_credential_scope() {
    let home = tempfile::tempdir().expect("temporary Codex home");
    let auth_manager = AuthManager::from_auth_for_testing_with_home(
        codex_login::CodexAuth::from_api_key("openai-token"),
        home.path().to_path_buf(),
    );
    let store = auth_manager.connection_store();
    let mut saved = Vec::new();
    for (name, token, machine) in [
        ("copilot a", "copilot-token-a", "a".repeat(64)),
        ("copilot b", "copilot-token-b", "b".repeat(64)),
    ] {
        let login = store
            .begin_login(
                ConnectionProvider::Copilot,
                NonEmptyString::new(name).expect("valid account name"),
            )
            .expect("begin saved Copilot login");
        std::fs::write(
            login.home().join("copilot-auth.json"),
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "github_token": token,
                "machine_id": machine,
            }))
            .expect("serialize Copilot credential"),
        )
        .expect("write Copilot credential");
        saved.push(login.finish().expect("finish saved Copilot login"));
    }

    let first = auth_manager
        .prepare_saved_connection(saved[0].id())
        .await
        .expect("prepare first account");
    auth_manager
        .activate_saved_connection(first)
        .await
        .expect("activate first account")
        .commit()
        .expect("commit first account");
    let first_manager = shared_endpoint_manager(Some(Arc::clone(&auth_manager)));
    let first_endpoint = first_manager.endpoint().await.expect("first endpoint");

    let second = auth_manager
        .prepare_saved_connection(saved[1].id())
        .await
        .expect("prepare second account");
    auth_manager
        .activate_saved_connection(second)
        .await
        .expect("activate second account")
        .commit()
        .expect("commit second account");
    let second_manager = shared_endpoint_manager(Some(Arc::clone(&auth_manager)));
    let second_endpoint = second_manager.endpoint().await.expect("second endpoint");

    assert!(!Arc::ptr_eq(&first_manager, &second_manager));
    assert_eq!(
        (
            first_endpoint
                .headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            second_endpoint
                .headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
        ),
        (Some("Bearer copilot-token-a"), Some("Bearer copilot-token-b"))
    );

    auth_manager.logout().await.expect("log out selected account");
    let error = second_manager
        .endpoint()
        .await
        .expect_err("logged-out credential must invalidate a warm endpoint");
    assert!(error.to_string().contains("requires"));
    assert!(!error.to_string().contains("copilot-token-b"));
}

#[tokio::test]
async fn models_timeout_includes_response_body_decoding() {
    use std::io::Read;
    use std::io::Write;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept models request");
        let mut request = [0u8; 4096];
        let mut received = 0;
        while received < request.len() {
            let read = stream
                .read(&mut request[received..])
                .expect("read models request");
            if read == 0 {
                break;
            }
            received += read;
            if request[..received]
                .windows(4)
                .any(|window| window == b"\r\n\r\n")
            {
                break;
            }
        }
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 128\r\n\r\n{\"data\":",
            )
            .expect("write partial models response");
        stream.flush().expect("flush partial models response");
        std::thread::sleep(Duration::from_millis(150));
    });
    let manager = Arc::new(manager_with_credential(credential(
        "copilot-secret",
        &format!("http://{address}"),
    )));
    let endpoint = CopilotModelsEndpoint::new(manager);

    let error = endpoint
        .list_models_with_timeout(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            Duration::from_millis(30),
        )
        .await
        .expect_err("partial response body should time out");

    assert_eq!(error.to_string(), CodexErr::Timeout.to_string());
    server.join().expect("join test server");
}

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
        credential_revision: Arc::new(|| 0),
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
) -> codex_protocol::error::Result<ModelsEndpointResponse> {
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
    let first_identity: Option<String> = manager.catalog_cache_policy().into();
    assert_eq!(first_identity, Some(first.catalog_identity.to_string()));
    manager.reject_generation(first.generation);
    assert_eq!(Option::<String>::from(manager.catalog_cache_policy()), None);
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
    assert_ne!(second.catalog_identity, first.catalog_identity);
    assert_eq!(
        Option::<String>::from(manager.catalog_cache_policy()),
        Some(second.catalog_identity.to_string())
    );
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

    let response = ModelsEndpointClient::list_models(
        &endpoint,
        "test-client",
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
    .expect("changed credential retries model discovery");

    assert_eq!(response.models.len(), 1);
    assert_eq!(response.models[0].slug, "gpt-5.6-sol");
    assert_eq!(response.etag, None);
    assert_eq!(Some(response.identity), endpoint.identity());
}

#[tokio::test]
async fn authenticated_catalog_cache_survives_endpoint_recreation() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
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
    let cache_home = tempfile::tempdir().expect("catalog cache directory");
    let cache_path = cache_home.path().join("copilot_models_cache.json");
    let first_endpoint = Arc::new(manager_with_credential(credential(
        "copilot-secret",
        &server.uri(),
    )));
    let first = OpenAiModelsManager::new_with_cache_path(
        cache_path.clone(),
        Arc::new(CopilotModelsEndpoint::new(first_endpoint)),
        /*auth_manager*/ None,
    );
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let expected = first
        .raw_model_catalog(RefreshStrategy::Online, factory.clone())
        .await;
    assert_eq!(expected.models.len(), 1);

    let restarted_endpoint = Arc::new(manager_with_credential(credential(
        "copilot-secret",
        &server.uri(),
    )));
    restarted_endpoint
        .endpoint()
        .await
        .expect("resolve restarted provider auth");
    let restarted = OpenAiModelsManager::new_with_cache_path(
        cache_path.clone(),
        Arc::new(CopilotModelsEndpoint::new(restarted_endpoint)),
        /*auth_manager*/ None,
    );
    assert_eq!(
        restarted
            .raw_model_catalog(RefreshStrategy::OnlineIfUncached, factory)
            .await,
        expected
    );
    assert!(
        !std::fs::read_to_string(cache_path)
            .expect("persisted catalog")
            .contains("copilot-secret")
    );
}

#[tokio::test]
async fn rejected_in_flight_credentials_cannot_publish_a_catalog() {
    let server = MockServer::start().await;
    let endpoint_manager = Arc::new(manager_with_credential(credential(
        "copilot-secret",
        &server.uri(),
    )));
    let snapshot = endpoint_manager.endpoint().await.expect("initial endpoint");
    let rejecting_manager = Arc::clone(&endpoint_manager);
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(move |_: &wiremock::Request| {
            rejecting_manager.reject_generation(snapshot.generation);
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{
                    "id": "gpt-5.6-sol",
                    "model_picker_enabled": true,
                    "supported_endpoints": ["ws:/responses"]
                }]
            }))
        })
        .expect(1)
        .mount(&server)
        .await;
    let cache_home = tempfile::tempdir().expect("catalog cache directory");
    let cache_path = cache_home.path().join("copilot_models_cache.json");
    let manager = OpenAiModelsManager::new_with_cache_path(
        cache_path.clone(),
        Arc::new(CopilotModelsEndpoint::new(endpoint_manager)),
        /*auth_manager*/ None,
    );

    assert_eq!(
        manager
            .raw_model_catalog(
                RefreshStrategy::Online,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await,
        codex_protocol::openai_models::ModelsResponse { models: Vec::new() }
    );
    assert!(!cache_path.exists());
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
async fn models_request_does_not_follow_redirects() {
    let redirect_target = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&redirect_target)
        .await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/models", redirect_target.uri())),
        )
        .expect(1)
        .mount(&server)
        .await;
    let manager = Arc::new(manager_with_credential(credential(
        "copilot-secret",
        &server.uri(),
    )));
    let endpoint = CopilotModelsEndpoint::new(manager);

    let error = ModelsEndpointClient::list_models(
        &endpoint,
        "test-client",
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
    .expect_err("Copilot model discovery must not follow redirects");

    assert_eq!(
        error.to_string(),
        "Fatal error: Copilot models request returned 302 Found"
    );
}

#[tokio::test]
async fn models_request_uses_http_path_and_returns_eligible_catalog() {
    let response = request_models(serde_json::json!({
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
        response
            .models
            .iter()
            .map(|model| model.slug.as_str())
            .collect::<Vec<_>>(),
        vec!["gpt-5.6-sol"]
    );
    assert_eq!(response.etag.as_deref(), Some("catalog-v1"));
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
    for base_url in [
        "https://openai.com",
        "https://api.openai.com/v1",
        "https://auth.api.openai.org",
        "https://api.openai.com.",
    ] {
        assert_eq!(
            validate_api_url(base_url),
            Err("Copilot API URL must not target an OpenAI domain".to_string()),
            "{base_url}"
        );
    }
    assert_eq!(
        validate_api_url("https://api.openai.com.example.test"),
        Ok("https://api.openai.com.example.test".to_string())
    );
}
