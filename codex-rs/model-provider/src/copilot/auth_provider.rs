use std::sync::Arc;

use codex_api::AuthError;
use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use serde_json::Map;
use serde_json::Value;

use super::endpoint::CopilotEndpointManager;
use super::endpoint::EndpointSnapshot;
use super::identity;
use super::payload;

const NO_REPOSITORY: &str = "__no_repository__";

/// Applies endpoint-owned Copilot Substrate identity to WebSocket upgrades and request frames.
pub(super) struct CopilotAuthProvider {
    endpoint: Arc<EndpointSnapshot>,
    endpoint_manager: Arc<CopilotEndpointManager>,
    request_identity: identity::RequestIdentity,
}

impl CopilotAuthProvider {
    pub(super) fn new(
        endpoint: Arc<EndpointSnapshot>,
        endpoint_manager: Arc<CopilotEndpointManager>,
        request_identity: identity::RequestIdentity,
    ) -> Self {
        Self {
            endpoint,
            endpoint_manager,
            request_identity,
        }
    }

    fn inject_headers(&self, headers: &mut HeaderMap) {
        remove_codex_headers(headers);
        headers.extend(self.endpoint.headers.clone());
        identity::prepare_inference_headers(headers);

        headers.insert(
            "x-initiator",
            HeaderValue::from_static(self.request_identity.initiator.as_str()),
        );
        headers.insert(
            "x-interaction-type",
            HeaderValue::from_static(self.request_identity.initiator.interaction_type()),
        );
        headers.insert(
            "x-agent-task-id",
            string_header(&self.request_identity.agent_task_id),
        );
        headers.insert(
            "x-client-session-id",
            string_header(&self.request_identity.client_session_id),
        );
        headers.insert(
            "x-interaction-id",
            string_header(&self.request_identity.interaction_id),
        );
        if let Some(parent_agent_id) = &self.request_identity.parent_agent_id {
            headers.insert("x-parent-agent-id", string_header(parent_agent_id));
        } else {
            headers.remove("x-parent-agent-id");
        }
        if let Some(client_machine_id) = self
            .endpoint
            .machine_id
            .as_ref()
            .or(self.request_identity.client_machine_id.as_ref())
        {
            headers.insert("x-client-machine-id", string_header(client_machine_id));
        } else {
            headers.remove("x-client-machine-id");
        }
        headers.insert(
            "x-github-repository-host",
            HeaderValue::from_static(NO_REPOSITORY),
        );
        headers.insert(
            "x-github-repository-nwo",
            HeaderValue::from_static(NO_REPOSITORY),
        );
        headers.insert(
            "x-stainless-helper-method",
            HeaderValue::from_static("stream"),
        );
    }

    fn frame_headers(&self) -> Map<String, Value> {
        let mut headers = Map::new();
        for (name, value) in [
            (
                "X-Interaction-Id",
                self.request_identity.interaction_id.as_str(),
            ),
            (
                "X-Interaction-Type",
                self.request_identity.initiator.interaction_type(),
            ),
            (
                "X-Agent-Task-Id",
                self.request_identity.agent_task_id.as_str(),
            ),
            (
                "X-Client-Session-Id",
                self.request_identity.client_session_id.as_str(),
            ),
            ("Copilot-Harness-Id", identity::HARNESS_ID),
        ] {
            headers.insert(name.to_string(), Value::String(value.to_string()));
        }
        if let Some(parent_agent_id) = &self.request_identity.parent_agent_id {
            headers.insert(
                "X-Parent-Agent-Id".to_string(),
                Value::String(parent_agent_id.clone()),
            );
        }
        headers
    }
}

impl std::fmt::Debug for CopilotAuthProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CopilotAuthProvider")
            .field("endpoint_generation", &self.endpoint.generation)
            .finish_non_exhaustive()
    }
}

impl AuthProvider for CopilotAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        self.inject_headers(headers);
    }

    fn prepare_responses_websocket_request(&self, request: String) -> Result<String, AuthError> {
        let mut value = serde_json::from_str::<Value>(&request).map_err(|error| {
            AuthError::Build(format!("decode Copilot WebSocket request: {error}"))
        })?;
        payload::normalize_websocket(&mut value);
        let object = value.as_object_mut().ok_or_else(|| {
            AuthError::Build("Copilot WebSocket request must be an object".to_string())
        })?;
        object.insert("headers".to_string(), Value::Object(self.frame_headers()));
        object.insert(
            "agent_task_id".to_string(),
            Value::String(self.request_identity.agent_task_id.clone()),
        );
        object.insert(
            "initiator".to_string(),
            Value::String(self.request_identity.initiator.as_str().to_string()),
        );
        serde_json::to_string(&value)
            .map_err(|error| AuthError::Build(format!("encode Copilot WebSocket request: {error}")))
    }

    fn on_responses_websocket_auth_rejected(&self) {
        self.endpoint_manager
            .reject_generation(self.endpoint.generation);
    }

    fn responses_websocket_connection_key(&self) -> Option<String> {
        Some(format!(
            "copilot-endpoint-{}-task-{}",
            self.endpoint.generation, self.request_identity.agent_task_id
        ))
    }
}

fn string_header(value: &str) -> HeaderValue {
    HeaderValue::from_str(value)
        .unwrap_or_else(|_| HeaderValue::from_static("00000000-0000-0000-0000-000000000000"))
}

fn remove_codex_headers(headers: &mut HeaderMap) {
    let names = headers
        .keys()
        .filter(|name| {
            let name = name.as_str();
            matches!(
                name,
                "session-id" | "thread-id" | "x-client-request-id" | "originator" | "openai-beta"
            ) || name.starts_with("x-codex-")
                || name.starts_with("x-oai-")
        })
        .cloned()
        .collect::<Vec<HeaderName>>();
    for name in names {
        headers.remove(name);
    }
}

#[cfg(test)]
#[path = "auth_provider_tests.rs"]
mod tests;
