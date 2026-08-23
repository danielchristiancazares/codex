use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;

use super::super::cli::CopilotEndpointManager;
use super::*;

#[test]
fn endpoint_rejection_targets_only_the_exact_generation() {
    let manager = CopilotEndpointManager::default();

    manager.reject_generation(7);

    assert!(manager.is_generation_rejected(7));
    assert!(!manager.is_generation_rejected(6));
    assert!(!manager.is_generation_rejected(8));
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
