use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use http::HeaderMap;
use http::HeaderName;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
const COPILOT_RPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Immutable endpoint material returned by the installed Copilot CLI.
#[derive(Clone)]
pub(super) struct EndpointSnapshot {
    pub(super) generation: u64,
    pub(super) base_url: String,
    pub(super) headers: HeaderMap,
    pub(super) bound_model: Option<String>,
}

impl fmt::Debug for EndpointSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header_names = self
            .headers
            .keys()
            .map(HeaderName::as_str)
            .collect::<Vec<_>>();
        formatter
            .debug_struct("EndpointSnapshot")
            .field("generation", &self.generation)
            .field("base_url", &self.base_url)
            .field("header_names", &header_names)
            .field("bound_model", &self.bound_model)
            .finish()
    }
}

#[derive(Debug)]
struct CachedEndpoint {
    snapshot: Arc<EndpointSnapshot>,
    refresh_at: Instant,
}

#[derive(Debug, Default)]
struct EndpointState {
    cached: HashMap<EndpointPurpose, CachedEndpoint>,
    generation: u64,
    refresh: HashMap<EndpointPurpose, Arc<OnceCell<Result<ResolvedEndpoint, String>>>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum EndpointPurpose {
    ModelsBroker,
    Inference { thread_id: ThreadId, model: String },
}

/// Resolves and refreshes inference endpoint snapshots through the native CLI.
#[derive(Debug)]
pub(super) struct CopilotEndpointManager {
    state: Mutex<EndpointState>,
    rejected_generations: StdMutex<HashSet<u64>>,
}

impl CopilotEndpointManager {
    pub(super) async fn endpoint(&self) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        self.endpoint_for_purpose(EndpointPurpose::ModelsBroker)
            .await
    }

    pub(super) async fn endpoint_for_model(
        &self,
        thread_id: ThreadId,
        model: &str,
    ) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        self.endpoint_for_purpose(EndpointPurpose::Inference {
            thread_id,
            model: model.to_string(),
        })
        .await
    }

    async fn endpoint_for_purpose(
        &self,
        purpose: EndpointPurpose,
    ) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        loop {
            let refresh = {
                let mut state = self.state.lock().await;
                if let Some(snapshot) = self.cached_snapshot(&state, &purpose) {
                    return Ok(snapshot);
                }
                Arc::clone(
                    state
                        .refresh
                        .entry(purpose.clone())
                        .or_insert_with(|| Arc::new(OnceCell::new())),
                )
            };

            let resolved = refresh
                .get_or_init(|| async {
                    tokio::time::timeout(COPILOT_RPC_TIMEOUT, async {
                        match &purpose {
                            EndpointPurpose::ModelsBroker => {
                                super::native_session::resolve_models_endpoint().await
                            }
                            EndpointPurpose::Inference { thread_id, model } => {
                                super::native_session::resolve_endpoint(*thread_id, model).await
                            }
                        }
                    })
                    .await
                    .map_err(|_| "Copilot CLI endpoint request timed out".to_string())?
                })
                .await
                .clone();
            let mut state = self.state.lock().await;
            if state
                .refresh
                .get(&purpose)
                .is_some_and(|active| Arc::ptr_eq(active, &refresh))
            {
                state.refresh.remove(&purpose);
                match resolved {
                    Ok(resolved) => {
                        state.generation = state.generation.saturating_add(1);
                        let snapshot = Arc::new(EndpointSnapshot {
                            generation: state.generation,
                            base_url: resolved.base_url,
                            headers: resolved.headers,
                            bound_model: resolved.bound_model,
                        });
                        let refresh_at = Instant::now()
                            .checked_add(resolved.refresh_after)
                            .unwrap_or_else(Instant::now);
                        state.cached.insert(
                            purpose.clone(),
                            CachedEndpoint {
                                snapshot: Arc::clone(&snapshot),
                                refresh_at,
                            },
                        );
                        self.prune_rejected_generations(&state);
                        return Ok(snapshot);
                    }
                    Err(error) => return Err(CodexErr::Fatal(error)),
                }
            }
            if let Some(snapshot) = self.cached_snapshot(&state, &purpose) {
                return Ok(snapshot);
            }
            if let Err(error) = resolved {
                return Err(CodexErr::Fatal(error));
            }
        }
    }

    fn cached_snapshot(
        &self,
        state: &EndpointState,
        purpose: &EndpointPurpose,
    ) -> Option<Arc<EndpointSnapshot>> {
        state.cached.get(purpose).and_then(|cached| {
            (!self.is_generation_rejected(cached.snapshot.generation)
                && Instant::now() < cached.refresh_at)
                .then(|| Arc::clone(&cached.snapshot))
        })
    }

    pub(super) fn reject_generation(&self, generation: u64) {
        self.rejected_generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(generation);
    }

    pub(super) fn is_generation_rejected(&self, generation: u64) -> bool {
        self.rejected_generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&generation)
    }

    fn prune_rejected_generations(&self, state: &EndpointState) {
        let cached_generations = state
            .cached
            .values()
            .map(|cached| cached.snapshot.generation)
            .collect::<HashSet<_>>();
        self.rejected_generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|generation| cached_generations.contains(generation));
    }
}

impl Default for CopilotEndpointManager {
    fn default() -> Self {
        Self {
            state: Mutex::new(EndpointState::default()),
            rejected_generations: StdMutex::new(HashSet::new()),
        }
    }
}

pub(super) fn shared_endpoint_manager() -> Arc<CopilotEndpointManager> {
    static MANAGER: OnceLock<Arc<CopilotEndpointManager>> = OnceLock::new();
    Arc::clone(MANAGER.get_or_init(|| Arc::new(CopilotEndpointManager::default())))
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedEndpoint {
    pub(super) base_url: String,
    pub(super) headers: HeaderMap,
    pub(super) bound_model: Option<String>,
    pub(super) refresh_after: Duration,
}
