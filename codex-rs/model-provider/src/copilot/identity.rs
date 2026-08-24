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

use crate::ProviderRequestContext;

/// Exact endpoint-facing wire identity expected by Copilot Substrate.
///
/// Native credential loading and transport preserve these values across model discovery and
/// inference.
pub(super) const CLIENT_APPLICATION: &str = "copilot-cli";
pub(super) const INTEGRATION_ID: &str = "copilot-developer-cli";
pub(super) const HARNESS_ID: &str = "copilot-sdk";
pub(super) const INTENT: &str = "conversation-agent";
pub(super) const USER_AGENT_VALUE: &str = "GitHubCopilotCLI/1.0.80";

const MAX_ACTIVE_THREAD_IDENTITIES: usize = 1_024;

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

/// Applies the static identity used for native Copilot API requests.
pub(super) fn prepare_inference_headers(headers: &mut HeaderMap) {
    headers.remove("editor-version");
    headers.remove("editor-plugin-version");
    headers.remove("x-github-api-version");
    headers.remove("x-request-id");
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
        HeaderName::from_static("x-client-application"),
        HeaderValue::from_static(CLIENT_APPLICATION),
    );
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
}
