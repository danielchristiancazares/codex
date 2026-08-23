use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn endpoint() -> Arc<EndpointSnapshot> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer endpoint-secret"),
    );
    headers.insert(
        "copilot-harness-id",
        HeaderValue::from_static("copilot-sdk"),
    );
    headers.insert(
        "copilot-integration-id",
        HeaderValue::from_static("copilot-developer-cli"),
    );
    headers.insert(
        "user-agent",
        HeaderValue::from_static("copilot/1.0.81-6 (win32 v24.18.1) term/unknown"),
    );
    headers.insert(
        "x-github-api-version",
        HeaderValue::from_static("2026-08-01"),
    );
    Arc::new(EndpointSnapshot {
        generation: 7,
        base_url: "https://api.githubcopilot.com".to_string(),
        headers,
        bound_model: None,
    })
}

fn manager() -> Arc<CopilotEndpointManager> {
    Arc::new(CopilotEndpointManager::default())
}

#[test]
fn websocket_connection_key_tracks_endpoint_generation() {
    let auth = CopilotAuthProvider::new(endpoint(), manager());

    assert_eq!(
        auth.responses_websocket_connection_key(),
        Some("copilot-endpoint-7".to_string())
    );
}

#[test]
fn websocket_upgrade_uses_current_copilot_cli_identity_headers() {
    let auth = CopilotAuthProvider::new(endpoint(), manager());
    let mut headers = HeaderMap::from_iter([
        (
            HeaderName::from_static("session-id"),
            HeaderValue::from_static("22222222-2222-2222-2222-222222222222"),
        ),
        (
            HeaderName::from_static("thread-id"),
            HeaderValue::from_static("codex-thread"),
        ),
        (
            HeaderName::from_static("openai-beta"),
            HeaderValue::from_static("responses_websockets=2026-02-06"),
        ),
        (
            HeaderName::from_static("x-codex-turn-metadata"),
            HeaderValue::from_static("codex-only"),
        ),
    ]);

    auth.add_auth_headers(&mut headers);

    assert_eq!(
        headers
            .get("copilot-integration-id")
            .and_then(|value| value.to_str().ok()),
        Some("vscode-chat")
    );
    assert_eq!(
        headers
            .get("x-client-application")
            .and_then(|value| value.to_str().ok()),
        Some("copilot-cli")
    );
    assert_eq!(
        headers
            .get("x-client-session-id")
            .and_then(|value| value.to_str().ok()),
        Some("22222222-2222-2222-2222-222222222222")
    );
    assert_eq!(
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        Some("copilot/1.0.81-6 (win32 v24.18.1) term/unknown")
    );
    assert_eq!(
        headers
            .get("openai-intent")
            .and_then(|value| value.to_str().ok()),
        Some("conversation-panel")
    );
    assert_eq!(headers.get("x-request-id"), headers.get("x-interaction-id"));
    assert_eq!(
        headers
            .get("x-initiator")
            .and_then(|value| value.to_str().ok()),
        Some("agent")
    );
    assert!(!headers.contains_key("copilot-harness-id"));
    assert!(!headers.contains_key("x-github-api-version"));
    assert!(!headers.contains_key("editor-version"));
    assert!(!headers.contains_key("editor-plugin-version"));
    assert!(!headers.contains_key("session-id"));
    assert!(!headers.contains_key("thread-id"));
    assert!(!headers.contains_key("openai-beta"));
    assert!(!headers.contains_key("x-codex-turn-metadata"));
}

#[test]
fn websocket_frame_carries_current_cli_header_envelope() {
    let auth = CopilotAuthProvider::new(endpoint(), manager());
    let mut upgrade_headers = HeaderMap::from_iter([(
        HeaderName::from_static("session-id"),
        HeaderValue::from_static("22222222-2222-2222-2222-222222222222"),
    )]);
    auth.add_auth_headers(&mut upgrade_headers);
    let agent_task_id = upgrade_headers
        .get("x-agent-task-id")
        .and_then(|value| value.to_str().ok())
        .expect("agent task id")
        .to_string();
    let upgrade_interaction_id = upgrade_headers
        .get("x-interaction-id")
        .and_then(|value| value.to_str().ok())
        .expect("interaction id")
        .to_string();
    let request = json!({
        "type": "response.create",
        "model": "gpt-5.6-sol",
        "input": [{"type": "message", "role": "assistant", "content": []}],
        "previous_response_id": "resp-1",
        "service_tier": "auto"
    });

    let prepared = auth
        .prepare_responses_websocket_request(request.to_string())
        .expect("prepare frame");
    let prepared: Value = serde_json::from_str(&prepared).expect("decode frame");
    let interaction_id = prepared["headers"]["X-Interaction-Id"]
        .as_str()
        .expect("frame interaction id")
        .to_string();

    assert_eq!(prepared["previous_response_id"], json!("resp-1"));
    assert_eq!(prepared["service_tier"], Value::Null);
    assert_ne!(interaction_id, upgrade_interaction_id);
    assert_eq!(
        prepared["headers"],
        json!({
            "Copilot-Integration-Id": "vscode-chat",
            "Openai-Intent": "conversation-panel",
            "X-Agent-Task-Id": agent_task_id,
            "X-Client-Application": "copilot-cli",
            "X-Client-Session-Id": "22222222-2222-2222-2222-222222222222",
            "X-Interaction-Id": interaction_id,
            "X-Interaction-Type": "conversation-agent",
            "X-Initiator": "agent",
            "X-Request-Id": interaction_id
        })
    );

    let next = auth
        .prepare_responses_websocket_request(request.to_string())
        .expect("prepare next frame");
    let next: Value = serde_json::from_str(&next).expect("decode next frame");
    assert_ne!(
        prepared["headers"]["X-Interaction-Id"],
        next["headers"]["X-Interaction-Id"]
    );
}
