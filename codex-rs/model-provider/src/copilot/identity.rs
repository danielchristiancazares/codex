use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::ACCEPT;
use http::header::CONTENT_TYPE;
use http::header::USER_AGENT;
use uuid::Uuid;

use super::endpoint::EndpointSource;
use crate::ProviderRequestContext;

pub(super) const CLIENT_APPLICATION: &str = "github/cli";
pub(super) const INTEGRATION_ID: &str = "copilot-developer-cli";
pub(super) const HARNESS_ID: &str = "copilot-sdk";
pub(super) const INTENT: &str = "conversation-agent";
pub(super) const EDITOR_VERSION: &str = "copilot/1.0.81-6";
pub(super) const API_VERSION: &str = "2026-08-01";
pub(super) const TOKEN_EXCHANGE_USER_AGENT: &str = "GitHubCopilotChat/1.0.0";
pub(super) const TOKEN_EXCHANGE_EDITOR_VERSION: &str = "Neovim/1.0.0";
pub(super) const TOKEN_EXCHANGE_EDITOR_PLUGIN_VERSION: &str = "CopilotChat/1.0.0";

const NODE_VERSION: &str = "v24.18.1";
const MAX_ACTIVE_THREAD_IDENTITIES: usize = 1_024;

#[cfg(target_os = "windows")]
const PLATFORM: &str = "win32";
#[cfg(target_os = "linux")]
const PLATFORM: &str = "linux";
#[cfg(target_os = "macos")]
const PLATFORM: &str = "darwin";
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
const PLATFORM: &str = std::env::consts::OS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Initiator {
    Agent,
    User,
}

impl Initiator {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::User => "user",
        }
    }

    pub(super) const fn interaction_type(self) -> &'static str {
        match self {
            Self::Agent => "conversation-subagent",
            Self::User => "conversation-user",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveThreadIdentity {
    turn_id: String,
    agent_task_id: String,
    generation: u64,
}

#[derive(Debug, Default)]
struct IdentityRegistry {
    active: HashMap<ThreadId, ActiveThreadIdentity>,
    generation: u64,
}

/// Copilot trajectory identity for one agent turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RequestIdentity {
    pub(super) agent_task_id: String,
    pub(super) parent_agent_id: Option<String>,
    pub(super) client_session_id: String,
    pub(super) client_machine_id: Option<String>,
    pub(super) interaction_id: String,
    pub(super) initiator: Initiator,
}

impl RequestIdentity {
    pub(super) fn new(
        request_context: &ProviderRequestContext,
        session_source: &SessionSource,
    ) -> Self {
        let initiator = if session_source.is_non_root_agent() {
            Initiator::Agent
        } else {
            Initiator::User
        };
        let ProviderRequestContext::Responses {
            installation_id,
            thread_id,
            turn_id,
            root_turn_id,
        } = request_context
        else {
            let agent_task_id = Uuid::new_v4().to_string();
            return Self {
                client_session_id: Uuid::new_v4().to_string(),
                client_machine_id: None,
                interaction_id: Uuid::new_v4().to_string(),
                agent_task_id,
                parent_agent_id: None,
                initiator,
            };
        };

        let (agent_task_id, parent_agent_id) =
            active_identity(*thread_id, turn_id, session_source.parent_thread_id());
        Self {
            agent_task_id,
            parent_agent_id,
            client_session_id: thread_id.to_string(),
            client_machine_id: Some(installation_id.clone()),
            interaction_id: root_turn_id.clone().unwrap_or_else(|| turn_id.clone()),
            initiator,
        }
    }
}

fn active_identity(
    thread_id: ThreadId,
    turn_id: &str,
    parent_thread_id: Option<ThreadId>,
) -> (String, Option<String>) {
    let registry = ACTIVE_IDENTITIES.get_or_init(|| Mutex::new(IdentityRegistry::default()));
    let mut registry = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    registry.generation = registry.generation.saturating_add(1);
    let generation = registry.generation;
    let parent_agent_id = parent_thread_id.and_then(|parent_thread_id| {
        registry
            .active
            .get(&parent_thread_id)
            .map(|identity| identity.agent_task_id.clone())
    });
    if let Some(identity) = registry.active.get_mut(&thread_id)
        && identity.turn_id == turn_id
    {
        identity.generation = generation;
        return (identity.agent_task_id.clone(), parent_agent_id);
    }

    if registry.active.len() >= MAX_ACTIVE_THREAD_IDENTITIES
        && !registry.active.contains_key(&thread_id)
        && let Some(oldest_thread_id) = registry
            .active
            .iter()
            .min_by_key(|(_, identity)| identity.generation)
            .map(|(thread_id, _)| *thread_id)
    {
        registry.active.remove(&oldest_thread_id);
    }
    let agent_task_id = Uuid::new_v4().to_string();
    registry.active.insert(
        thread_id,
        ActiveThreadIdentity {
            turn_id: turn_id.to_string(),
            agent_task_id: agent_task_id.clone(),
            generation,
        },
    );
    (agent_task_id, parent_agent_id)
}

static ACTIVE_IDENTITIES: OnceLock<Mutex<IdentityRegistry>> = OnceLock::new();

/// Applies the static identity appropriate to the source of a Copilot endpoint.
pub(super) fn prepare_inference_headers(headers: &mut HeaderMap, source: EndpointSource) {
    headers.remove("editor-plugin-version");
    headers.remove("x-client-application");
    headers.remove("x-request-id");
    match source {
        EndpointSource::Direct => {
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(
                HeaderName::from_static("copilot-integration-id"),
                HeaderValue::from_static(INTEGRATION_ID),
            );
            headers.insert(
                HeaderName::from_static("copilot-harness-id"),
                HeaderValue::from_static(HARNESS_ID),
            );
            headers.insert(
                HeaderName::from_static("openai-intent"),
                HeaderValue::from_static(INTENT),
            );
            headers.insert(
                HeaderName::from_static("editor-version"),
                HeaderValue::from_static(EDITOR_VERSION),
            );
            headers.insert(
                HeaderName::from_static("x-github-api-version"),
                HeaderValue::from_static(API_VERSION),
            );
            if let Ok(value) = HeaderValue::from_str(&format!(
                "copilot/1.0.81-6 ({PLATFORM} {NODE_VERSION}) term/unknown client/{CLIENT_APPLICATION}"
            )) {
                headers.insert(USER_AGENT, value);
            }
        }
        // The CLI endpoint response owns its version, platform, and runtime identity.
        EndpointSource::Cli => append_cli_client_to_user_agent(headers),
    }
}

fn append_cli_client_to_user_agent(headers: &mut HeaderMap) {
    let Some(user_agent) = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
    else {
        return;
    };
    if user_agent.contains("client/github/cli") {
        return;
    }
    if let Ok(value) = HeaderValue::from_str(&format!("{user_agent} client/{CLIENT_APPLICATION}")) {
        headers.insert(USER_AGENT, value);
    }
}
