use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_protocol::error::CodexErr;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

use super::credentials::CopilotCredential;
use super::credentials::CopilotCredentialSource;
use super::credentials::CredentialLoadError;

type CredentialLoader =
    Arc<dyn Fn() -> Result<CopilotCredential, CredentialLoadError> + Send + Sync + 'static>;

/// Immutable endpoint material obtained from native GitHub authentication.
#[derive(Clone)]
pub(super) struct EndpointSnapshot {
    pub(super) generation: u64,
    pub(super) base_url: String,
    pub(super) headers: HeaderMap,
    pub(super) machine_id: Option<String>,
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
            .field("machine_id", &self.machine_id)
            .finish()
    }
}

struct CachedEndpoint {
    snapshot: Arc<EndpointSnapshot>,
    credential: CopilotCredential,
}

impl fmt::Debug for CachedEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CachedEndpoint")
            .field("snapshot", &self.snapshot)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct EndpointState {
    cached: Option<CachedEndpoint>,
    generation: u64,
    refresh: Option<Arc<OnceCell<Result<CopilotCredential, String>>>>,
}

impl fmt::Debug for EndpointState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EndpointState")
            .field("cached", &self.cached)
            .field("generation", &self.generation)
            .field("refresh_in_progress", &self.refresh.is_some())
            .finish()
    }
}

/// Resolves and caches direct Copilot Substrate credentials.
pub(super) struct CopilotEndpointManager {
    state: Mutex<EndpointState>,
    current_generation: AtomicU64,
    rejected_generations: StdMutex<HashSet<u64>>,
    credential_loader: CredentialLoader,
}

impl fmt::Debug for CopilotEndpointManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CopilotEndpointManager")
            .field("state", &self.state)
            .field(
                "current_generation",
                &self.current_generation.load(Ordering::Acquire),
            )
            .field("rejected_generations", &self.rejected_generations)
            .finish_non_exhaustive()
    }
}

impl CopilotEndpointManager {
    pub(super) async fn endpoint(&self) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        loop {
            let refresh = {
                let mut state = self.state.lock().await;
                if let Some(snapshot) = self.cached_snapshot(&state) {
                    return Ok(snapshot);
                }
                Arc::clone(
                    state
                        .refresh
                        .get_or_insert_with(|| Arc::new(OnceCell::new())),
                )
            };

            let credential = refresh.get_or_init(|| self.load_credential()).await.clone();
            let mut state = self.state.lock().await;
            let owns_refresh = state
                .refresh
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(active, &refresh));
            if owns_refresh {
                state.refresh = None;
                return match credential {
                    Ok(credential) => self.install_credential(&mut state, credential),
                    Err(error) => Err(CodexErr::Fatal(error)),
                };
            }
            if let Some(snapshot) = self.cached_snapshot(&state) {
                return Ok(snapshot);
            }
            match credential {
                Ok(credential)
                    if state.cached.as_ref().is_some_and(|cached| {
                        self.is_generation_rejected(cached.snapshot.generation)
                            && cached.credential == credential
                    }) =>
                {
                    return Err(rejected_credential_error(credential.source));
                }
                Ok(_) => {}
                Err(error) => return Err(CodexErr::Fatal(error)),
            }
        }
    }

    async fn load_credential(&self) -> Result<CopilotCredential, String> {
        let credential_loader = Arc::clone(&self.credential_loader);
        tokio::task::spawn_blocking(move || credential_loader())
            .await
            .map_err(|error| format!("load GitHub credential for Copilot: {error}"))?
            .map_err(|error| error.to_string())
    }

    fn install_credential(
        &self,
        state: &mut EndpointState,
        credential: CopilotCredential,
    ) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        if state.cached.as_ref().is_some_and(|cached| {
            self.is_generation_rejected(cached.snapshot.generation)
                && cached.credential == credential
        }) {
            return Err(rejected_credential_error(credential.source));
        }

        let resolved = resolved_direct_endpoint(&credential).map_err(CodexErr::Fatal)?;
        state.generation = state.generation.saturating_add(1);
        let snapshot = Arc::new(EndpointSnapshot {
            generation: state.generation,
            base_url: resolved.base_url,
            headers: resolved.headers,
            machine_id: resolved.machine_id,
        });
        state.cached = Some(CachedEndpoint {
            snapshot: Arc::clone(&snapshot),
            credential,
        });
        self.current_generation
            .store(snapshot.generation, Ordering::Release);
        self.prune_rejected_generations(snapshot.generation);
        Ok(snapshot)
    }

    fn cached_snapshot(&self, state: &EndpointState) -> Option<Arc<EndpointSnapshot>> {
        let cached = state.cached.as_ref()?;
        self.prune_rejected_generations(cached.snapshot.generation);
        (!self.is_generation_rejected(cached.snapshot.generation))
            .then(|| Arc::clone(&cached.snapshot))
    }

    pub(super) fn reject_generation(&self, generation: u64) {
        if self.current_generation.load(Ordering::Acquire) == generation {
            self.rejected_generations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(generation);
        }
    }

    pub(super) fn is_generation_rejected(&self, generation: u64) -> bool {
        self.rejected_generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&generation)
    }

    fn prune_rejected_generations(&self, current_generation: u64) {
        self.rejected_generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|generation| *generation == current_generation);
    }
}

impl Default for CopilotEndpointManager {
    fn default() -> Self {
        Self {
            state: Mutex::new(EndpointState::default()),
            current_generation: AtomicU64::new(0),
            rejected_generations: StdMutex::new(HashSet::new()),
            credential_loader: Arc::new(super::credentials::load),
        }
    }
}

pub(super) fn shared_endpoint_manager() -> Arc<CopilotEndpointManager> {
    static MANAGER: OnceLock<Arc<CopilotEndpointManager>> = OnceLock::new();
    Arc::clone(MANAGER.get_or_init(|| Arc::new(CopilotEndpointManager::default())))
}

struct ResolvedEndpoint {
    base_url: String,
    headers: HeaderMap,
    machine_id: Option<String>,
}

fn resolved_direct_endpoint(credential: &CopilotCredential) -> Result<ResolvedEndpoint, String> {
    let mut headers = HeaderMap::new();
    let mut authorization = HeaderValue::from_str(&format!("Bearer {}", credential.token))
        .map_err(|error| format!("encode Copilot Substrate credential: {error}"))?;
    authorization.set_sensitive(true);
    headers.insert(AUTHORIZATION, authorization);
    Ok(ResolvedEndpoint {
        base_url: validate_api_url(&credential.base_url)?,
        headers,
        machine_id: credential.machine_id.clone(),
    })
}

fn validate_api_url(base_url: &str) -> Result<String, String> {
    let uri = base_url
        .parse::<http::Uri>()
        .map_err(|_| "configured Copilot API URL is invalid".to_string())?;
    let scheme = uri.scheme_str();
    let authority = uri
        .authority()
        .ok_or_else(|| "configured Copilot API URL is invalid".to_string())?;
    let host = uri
        .host()
        .ok_or_else(|| "configured Copilot API URL is invalid".to_string())?;
    if authority.as_str().contains('@')
        || uri
            .path_and_query()
            .and_then(http::uri::PathAndQuery::query)
            .is_some()
    {
        return Err("configured Copilot API URL is invalid".to_string());
    }
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    let normalized_host = host.trim_end_matches('.').to_ascii_lowercase();
    let is_openai_host = ["openai.com", "openai.org"].into_iter().any(|domain| {
        normalized_host == domain
            || normalized_host
                .strip_suffix(domain)
                .is_some_and(|prefix| prefix.ends_with('.'))
    });
    if is_openai_host {
        return Err("Copilot API URL must not target an OpenAI domain".to_string());
    }
    if scheme != Some("https") && !(scheme == Some("http") && loopback) {
        return Err("Copilot API URL must use HTTPS unless it targets loopback".to_string());
    }
    Ok(base_url.trim_end_matches('/').to_string())
}

fn rejected_credential_error(source: CopilotCredentialSource) -> CodexErr {
    let guidance = match source {
        CopilotCredentialSource::ApiTokenEnvironment => {
            "update `GITHUB_COPILOT_API_TOKEN` before retrying"
        }
        CopilotCredentialSource::DedicatedGitHubEnvironment => {
            "update `COPILOT_GITHUB_TOKEN` before retrying"
        }
        CopilotCredentialSource::StoredOAuth => {
            "run `codex login --provider copilot --force` before retrying"
        }
        CopilotCredentialSource::GhTokenEnvironment => "update `GH_TOKEN` before retrying",
        CopilotCredentialSource::GitHubTokenEnvironment => "update `GITHUB_TOKEN` before retrying",
    };
    CodexErr::Fatal(format!(
        "GitHub Copilot Substrate rejected the current credential; {guidance}"
    ))
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
