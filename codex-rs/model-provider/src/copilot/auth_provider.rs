use std::sync::Arc;
use std::sync::Mutex;

use codex_api::AuthError;
use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use serde_json::Map;
use serde_json::Value;
use uuid::Uuid;

use super::endpoint::CopilotEndpointManager;
use super::endpoint::EndpointSnapshot;
use super::endpoint::EndpointSource;
use super::identity;
use super::payload;

const NO_REPOSITORY: &str = "__no_repository__";

#[derive(Clone, Debug)]
struct ConnectionIdentity {
    agent_task_id: HeaderValue,
    client_session_id: HeaderValue,
}

/// Applies endpoint-owned Copilot identity to the WebSocket upgrade and request frames.
pub(super) struct CopilotAuthProvider {
    endpoint: Arc<EndpointSnapshot>,
    endpoint_manager: Arc<CopilotEndpointManager>,
    connection: Mutex<Option<ConnectionIdentity>>,
}

impl CopilotAuthProvider {
    pub(super) fn new(
        endpoint: Arc<EndpointSnapshot>,
        endpoint_manager: Arc<CopilotEndpointManager>,
    ) -> Self {
        Self {
            endpoint,
            endpoint_manager,
            connection: Mutex::new(None),
        }
    }

    fn inject_headers(&self, headers: &mut HeaderMap) {
        let client_session_id = headers
            .get("session-id")
            .cloned()
            .unwrap_or_else(new_uuid_header);
        remove_codex_headers(headers);
        headers.extend(self.endpoint.headers.clone());
        identity::prepare_inference_headers(headers, self.endpoint.source);

        let identity = ConnectionIdentity {
            agent_task_id: new_uuid_header(),
            client_session_id,
        };
        let interaction_id = new_uuid_header();
        headers.insert("x-initiator", HeaderValue::from_static("agent"));
        headers.insert(
            "x-interaction-type",
            HeaderValue::from_static("conversation-agent"),
        );
        headers.insert("x-agent-task-id", identity.agent_task_id.clone());
        headers.insert("x-client-session-id", identity.client_session_id.clone());
        headers.insert("x-interaction-id", interaction_id.clone());
        headers.insert("x-request-id", interaction_id);
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
        *self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(identity);
    }

    fn frame_headers(&self, initiator: payload::Initiator) -> Map<String, Value> {
        let identity = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| ConnectionIdentity {
                agent_task_id: new_uuid_header(),
                client_session_id: new_uuid_header(),
            });
        let interaction_id = new_uuid_header();
        let mut headers = Map::new();
        headers.insert(
            "Copilot-Integration-Id".to_string(),
            Value::String(identity::INTEGRATION_ID.to_string()),
        );
        headers.insert(
            "Openai-Intent".to_string(),
            Value::String(identity::INTENT.to_string()),
        );
        if self.endpoint.source == EndpointSource::Direct {
            headers.insert(
                "Editor-Version".to_string(),
                Value::String(identity::EDITOR_VERSION.to_string()),
            );
            headers.insert(
                "Editor-Plugin-Version".to_string(),
                Value::String(identity::EDITOR_PLUGIN_VERSION.to_string()),
            );
        }
        headers.insert(
            "X-Client-Application".to_string(),
            Value::String(identity::CLIENT_APPLICATION.to_string()),
        );
        insert_json_header(&mut headers, "X-Agent-Task-Id", &identity.agent_task_id);
        insert_json_header(
            &mut headers,
            "X-Client-Session-Id",
            &identity.client_session_id,
        );
        insert_json_header(&mut headers, "X-Interaction-Id", &interaction_id);
        insert_json_header(&mut headers, "X-Request-Id", &interaction_id);
        headers.insert(
            "X-Initiator".to_string(),
            Value::String(initiator.as_str().to_string()),
        );
        headers.insert(
            "X-Interaction-Type".to_string(),
            Value::String(initiator.interaction_type().to_string()),
        );
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
        let initiator = payload::initiator(&value);
        payload::normalize_websocket(&mut value);
        let object = value.as_object_mut().ok_or_else(|| {
            AuthError::Build("Copilot WebSocket request must be an object".to_string())
        })?;
        object.insert(
            "headers".to_string(),
            Value::Object(self.frame_headers(initiator)),
        );
        serde_json::to_string(&value)
            .map_err(|error| AuthError::Build(format!("encode Copilot WebSocket request: {error}")))
    }

    fn on_responses_websocket_auth_rejected(&self) {
        self.endpoint_manager
            .reject_generation(self.endpoint.generation);
    }

    fn responses_websocket_connection_key(&self) -> Option<String> {
        Some(format!("copilot-endpoint-{}", self.endpoint.generation))
    }
}

fn new_uuid_header() -> HeaderValue {
    HeaderValue::from_str(&Uuid::new_v4().to_string())
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

fn insert_json_header(output: &mut Map<String, Value>, name: &str, value: &HeaderValue) {
    if let Ok(value) = value.to_str() {
        output.insert(name.to_string(), Value::String(value.to_string()));
    }
}

#[cfg(test)]
#[path = "auth_provider_tests.rs"]
mod tests;
