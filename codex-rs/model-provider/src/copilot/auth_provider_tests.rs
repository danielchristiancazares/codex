use std::collections::BTreeMap;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;
use crate::ProviderRequestContext;

const TEST_MACHINE_ID: &str = "4f8c2f5df054b1e465c8f9d9af3b391a4718b02ad7c3d0f8e83d4f6978de1451";

fn endpoint() -> Arc<EndpointSnapshot> {
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("accept", "application/json"),
        ("content-type", "application/json"),
        ("copilot-harness-id", "copilot-sdk"),
        ("copilot-integration-id", "copilot-developer-cli"),
        ("editor-plugin-version", "stale-plugin"),
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
        machine_id: Some(TEST_MACHINE_ID.to_string()),
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
    let auth = CopilotAuthProvider::new(endpoint(), manager(), request_identity);

    assert_eq!(auth.responses_websocket_connection_key(), Some(expected));
}

#[test]
fn websocket_upgrade_matches_copilot_substrate_identity() {
    let thread_id = ThreadId::new();
    let thread_id_string = thread_id.to_string();
    let turn_id = "22222222-2222-4222-8222-222222222222";
    let request_identity = root_identity(thread_id, turn_id);
    let agent_task_id = request_identity.agent_task_id.clone();
    let auth = CopilotAuthProvider::new(endpoint(), manager(), request_identity);
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

    let actual = headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().expect("ASCII header").to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual,
        BTreeMap::from([
            ("accept".to_string(), "application/json".to_string()),
            (
                "authorization".to_string(),
                "Bearer endpoint-secret".to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
            ("copilot-harness-id".to_string(), "copilot-sdk".to_string(),),
            (
                "copilot-integration-id".to_string(),
                "copilot-developer-cli".to_string(),
            ),
            (
                "openai-intent".to_string(),
                "conversation-agent".to_string(),
            ),
            (
                "user-agent".to_string(),
                "GitHubCopilotCLI/1.0.80".to_string(),
            ),
            ("x-agent-task-id".to_string(), agent_task_id),
            (
                "x-client-machine-id".to_string(),
                TEST_MACHINE_ID.to_string(),
            ),
            ("x-client-session-id".to_string(), thread_id_string),
            (
                "x-client-application".to_string(),
                "copilot-cli".to_string(),
            ),
            (
                "x-github-repository-host".to_string(),
                "__no_repository__".to_string(),
            ),
            (
                "x-github-repository-nwo".to_string(),
                "__no_repository__".to_string(),
            ),
            ("x-initiator".to_string(), "user".to_string()),
            ("x-interaction-id".to_string(), turn_id.to_string()),
            (
                "x-interaction-type".to_string(),
                "conversation-user".to_string(),
            ),
            (
                "x-stainless-helper-method".to_string(),
                "stream".to_string(),
            ),
        ])
    );
}

#[test]
fn websocket_frame_matches_copilot_substrate_envelope() {
    let thread_id = ThreadId::new();
    let turn_id = "44444444-4444-4444-8444-444444444444";
    let request_identity = root_identity(thread_id, turn_id);
    let agent_task_id = request_identity.agent_task_id.clone();
    let auth = CopilotAuthProvider::new(endpoint(), manager(), request_identity);
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

    let auth = CopilotAuthProvider::new(endpoint(), manager(), request_identity.clone());
    let retry_auth = CopilotAuthProvider::new(endpoint(), manager(), retry_identity);
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
