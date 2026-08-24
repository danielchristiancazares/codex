use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::super::endpoint::EndpointSource;
use super::*;
use crate::ProviderRequestContext;

fn endpoint(source: EndpointSource) -> Arc<EndpointSnapshot> {
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("accept", "application/json"),
        ("content-type", "application/json"),
        ("copilot-harness-id", "copilot-sdk"),
        ("copilot-integration-id", "copilot-developer-cli"),
        ("editor-version", "copilot/1.0.81-6"),
        ("openai-intent", "conversation-agent"),
        (
            "user-agent",
            "copilot/1.0.81-6 (win32 v24.18.1) term/unknown",
        ),
        ("x-github-api-version", "2026-08-01"),
        ("x-initiator", "user"),
        ("x-interaction-id", "11111111-1111-1111-1111-111111111111"),
    ] {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer endpoint-secret"),
    );
    Arc::new(EndpointSnapshot {
        generation: 7,
        base_url: "https://api.githubcopilot.com".to_string(),
        headers,
        bound_model: None,
        source,
    })
}

fn manager() -> Arc<CopilotEndpointManager> {
    Arc::new(CopilotEndpointManager::default())
}

fn request_context(
    thread_id: ThreadId,
    turn_id: &str,
    root_turn_id: Option<&str>,
) -> ProviderRequestContext {
    ProviderRequestContext::Responses {
        installation_id: "99999999-9999-4999-8999-999999999999".to_string(),
        thread_id,
        turn_id: turn_id.to_string(),
        root_turn_id: root_turn_id.map(ToString::to_string),
    }
}

fn root_identity(thread_id: ThreadId, turn_id: &str) -> identity::RequestIdentity {
    identity::RequestIdentity::new(
        &request_context(thread_id, turn_id, /*root_turn_id*/ None),
        &SessionSource::Cli,
    )
}

#[test]
fn websocket_connection_key_tracks_endpoint_and_agent_turn() {
    let request_identity = root_identity(ThreadId::new(), "22222222-2222-4222-8222-222222222222");
    let expected = format!("copilot-endpoint-7-task-{}", request_identity.agent_task_id);
    let auth = CopilotAuthProvider::new(
        endpoint(EndpointSource::Direct),
        manager(),
        request_identity,
    );

    assert_eq!(auth.responses_websocket_connection_key(), Some(expected));
}

#[test]
fn websocket_upgrade_uses_copilot_cli_identity() {
    let thread_id = ThreadId::new();
    let thread_id_string = thread_id.to_string();
    let turn_id = "22222222-2222-4222-8222-222222222222";
    let request_identity = root_identity(thread_id, turn_id);
    let agent_task_id = request_identity.agent_task_id.clone();
    let auth = CopilotAuthProvider::new(
        endpoint(EndpointSource::Direct),
        manager(),
        request_identity,
    );
    let mut headers = HeaderMap::from_iter([
        (
            HeaderName::from_static("session-id"),
            HeaderValue::from_static("33333333-3333-4333-8333-333333333333"),
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

    let actual = [
        "copilot-integration-id",
        "copilot-harness-id",
        "openai-intent",
        "x-agent-task-id",
        "x-client-session-id",
        "x-client-machine-id",
        "x-interaction-id",
        "x-initiator",
        "x-interaction-type",
        "x-github-api-version",
        "editor-version",
        "x-github-repository-host",
        "x-github-repository-nwo",
        "x-stainless-helper-method",
    ]
    .map(|name| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string)
    });
    assert_eq!(
        actual,
        [
            Some("copilot-developer-cli".to_string()),
            Some("copilot-sdk".to_string()),
            Some("conversation-agent".to_string()),
            Some(agent_task_id),
            Some(thread_id_string),
            Some("99999999-9999-4999-8999-999999999999".to_string()),
            Some(turn_id.to_string()),
            Some("user".to_string()),
            Some("conversation-user".to_string()),
            Some("2026-08-01".to_string()),
            Some("copilot/1.0.81-6".to_string()),
            Some("__no_repository__".to_string()),
            Some("__no_repository__".to_string()),
            Some("stream".to_string()),
        ]
    );
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .expect("user agent");
    assert!(user_agent.starts_with("copilot/1.0.81-6 ("));
    assert!(user_agent.ends_with("term/unknown client/github/cli"));
    for removed in [
        "editor-plugin-version",
        "x-request-id",
        "x-client-application",
        "x-parent-agent-id",
        "session-id",
        "thread-id",
        "openai-beta",
        "x-codex-turn-metadata",
    ] {
        assert!(!headers.contains_key(removed));
    }
}

#[test]
fn websocket_frame_matches_copilot_cli_envelope() {
    let thread_id = ThreadId::new();
    let turn_id = "44444444-4444-4444-8444-444444444444";
    let request_identity = root_identity(thread_id, turn_id);
    let agent_task_id = request_identity.agent_task_id.clone();
    let auth = CopilotAuthProvider::new(
        endpoint(EndpointSource::Direct),
        manager(),
        request_identity,
    );
    let request = json!({
        "type": "response.create",
        "model": "gpt-5.6-sol",
        "input": [{"type": "message", "role": "user", "content": []}],
        "previous_response_id": "resp-1",
        "service_tier": "auto",
        "stream": true,
        "tool_choice": "auto"
    });

    let prepared = auth
        .prepare_responses_websocket_request(request.to_string())
        .expect("prepare frame");
    let prepared: Value = serde_json::from_str(&prepared).expect("decode frame");
    assert_eq!(
        prepared,
        json!({
            "agent_task_id": agent_task_id,
            "headers": {
                "Copilot-Harness-Id": "copilot-sdk",
                "X-Agent-Task-Id": agent_task_id,
                "X-Client-Session-Id": thread_id,
                "X-Interaction-Id": turn_id,
                "X-Interaction-Type": "conversation-user"
            },
            "initiator": "user",
            "input": [{"type": "message", "role": "user", "content": []}],
            "model": "gpt-5.6-sol",
            "previous_response_id": "resp-1",
            "type": "response.create"
        })
    );
}

#[test]
fn cli_endpoint_identity_is_preserved_and_completed_for_inference() {
    let auth = CopilotAuthProvider::new(
        endpoint(EndpointSource::Cli),
        manager(),
        root_identity(ThreadId::new(), "55555555-5555-4555-8555-555555555555"),
    );
    let mut headers = HeaderMap::new();

    auth.add_auth_headers(&mut headers);

    let actual = [
        "user-agent",
        "editor-version",
        "openai-intent",
        "copilot-integration-id",
        "copilot-harness-id",
        "x-github-api-version",
    ]
    .map(|name| headers.get(name).and_then(|value| value.to_str().ok()));
    assert_eq!(
        actual,
        [
            Some("copilot/1.0.81-6 (win32 v24.18.1) term/unknown client/github/cli"),
            Some("copilot/1.0.81-6"),
            Some("conversation-agent"),
            Some("copilot-developer-cli"),
            Some("copilot-sdk"),
            Some("2026-08-01"),
        ]
    );
}

#[test]
fn child_turn_uses_parent_agent_task_lineage_across_retries() {
    let parent_thread_id = ThreadId::new();
    let child_thread_id = ThreadId::new();
    let root_turn_id = "66666666-6666-4666-8666-666666666666";
    let parent_identity = root_identity(parent_thread_id, root_turn_id);
    let child_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id,
        depth: 1,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    });
    let context = request_context(
        child_thread_id,
        "77777777-7777-4777-8777-777777777777",
        Some(root_turn_id),
    );
    let request_identity = identity::RequestIdentity::new(&context, &child_source);
    let retry_identity = identity::RequestIdentity::new(&context, &child_source);
    assert_eq!(request_identity, retry_identity);
    assert_eq!(
        request_identity.parent_agent_id.as_deref(),
        Some(parent_identity.agent_task_id.as_str())
    );

    let auth = CopilotAuthProvider::new(
        endpoint(EndpointSource::Cli),
        manager(),
        request_identity.clone(),
    );
    let retry_auth =
        CopilotAuthProvider::new(endpoint(EndpointSource::Cli), manager(), retry_identity);
    let request = json!({
        "type": "response.create",
        "model": "gpt-5.6-sol",
        "input": []
    })
    .to_string();
    let prepare = |auth: &CopilotAuthProvider| {
        let mut upgrade = HeaderMap::new();
        auth.add_auth_headers(&mut upgrade);
        let frame = auth
            .prepare_responses_websocket_request(request.clone())
            .expect("prepare child frame");
        let frame: Value = serde_json::from_str(&frame).expect("decode child frame");
        (upgrade, frame)
    };

    let (upgrade, frame) = prepare(&auth);
    let (retry_upgrade, retry_frame) = prepare(&retry_auth);
    assert_eq!(upgrade, retry_upgrade);
    assert_eq!(frame, retry_frame);
    assert_eq!(
        frame["headers"],
        json!({
            "Copilot-Harness-Id": "copilot-sdk",
            "X-Agent-Task-Id": request_identity.agent_task_id,
            "X-Client-Session-Id": child_thread_id,
            "X-Interaction-Id": root_turn_id,
            "X-Interaction-Type": "conversation-subagent",
            "X-Parent-Agent-Id": parent_identity.agent_task_id
        })
    );
    assert_eq!(frame["initiator"], "agent");
    assert_eq!(
        upgrade
            .get("x-parent-agent-id")
            .and_then(|value| value.to_str().ok()),
        Some(parent_identity.agent_task_id.as_str())
    );
}
