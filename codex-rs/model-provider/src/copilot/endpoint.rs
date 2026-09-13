use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_login::AuthManager;
use codex_protocol::error::CodexErr;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

use super::catalog_identity::CatalogCachePolicy;
use super::catalog_identity::CatalogIdentity;
use super::credentials::CopilotCredential;
use super::credentials::CopilotCredentialSource;
use super::credentials::CredentialLoadError;

type CredentialLoader =
    Arc<dyn Fn() -> Result<CopilotCredential, CredentialLoadError> + Send + Sync + 'static>;
type CredentialRevision = Arc<dyn Fn() -> u64 + Send + Sync + 'static>;

#[derive(Clone)]
struct LoadedCredential {
    credential: CopilotCredential,
    revision: u64,
}

/// Immutable endpoint material obtained from native GitHub authentication.
#[derive(Clone)]
pub(super) struct EndpointSnapshot {
    pub(super) generation: u64,
    pub(super) catalog_identity: CatalogIdentity,
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
    credential_revision: u64,
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
    refresh: Option<Arc<OnceCell<Result<LoadedCredential, String>>>>,
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
    credential_revision: CredentialRevision,
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
    /// Observe installed auth without blocking on credential storage or an active refresh.
    ///
    /// A cold or rejected endpoint must authenticate before its disk catalog can be reused.
    pub(super) fn catalog_cache_policy(&self) -> CatalogCachePolicy {
        match self.state.try_lock() {
            Ok(state) => match self.cached_snapshot(&state) {
                Some(snapshot) => {
                    CatalogCachePolicy::ReuseAuthenticatedCatalog(snapshot.catalog_identity.clone())
                }
                None => CatalogCachePolicy::FetchAuthenticatedCatalog,
            },
            Err(_) => CatalogCachePolicy::FetchAuthenticatedCatalog,
        }
    }

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

            let loaded = refresh.get_or_init(|| self.load_credential()).await.clone();
            let mut state = self.state.lock().await;
            let owns_refresh = state
                .refresh
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(active, &refresh));
            if owns_refresh {
                state.refresh = None;
                match loaded {
                    Ok(loaded) if loaded.revision == (self.credential_revision)() => {
                        return self.install_credential(&mut state, loaded);
                    }
                    Ok(_) => continue,
                    Err(error) => return Err(CodexErr::Fatal(error)),
                }
            }
            if let Some(snapshot) = self.cached_snapshot(&state) {
                return Ok(snapshot);
            }
            match loaded {
                Ok(loaded)
                    if state.cached.as_ref().is_some_and(|cached| {
                        self.is_generation_rejected(cached.snapshot.generation)
                            && cached.credential == loaded.credential
                    }) =>
                {
                    return Err(rejected_credential_error(loaded.credential.source));
                }
                Ok(_) => {}
                Err(error) => return Err(CodexErr::Fatal(error)),
            }
        }
    }

    async fn load_credential(&self) -> Result<LoadedCredential, String> {
        let revision = (self.credential_revision)();
        let credential_loader = Arc::clone(&self.credential_loader);
        let credential = tokio::task::spawn_blocking(move || credential_loader())
            .await
            .map_err(|error| format!("load GitHub credential for Copilot: {error}"))?
            .map_err(|error| error.to_string())?;
        Ok(LoadedCredential {
            credential,
            revision,
        })
    }

    fn install_credential(
        &self,
        state: &mut EndpointState,
        loaded: LoadedCredential,
    ) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        let LoadedCredential {
            credential,
            revision,
        } = loaded;
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
            catalog_identity: CatalogIdentity::for_credential(&credential),
            base_url: resolved.base_url,
            headers: resolved.headers,
            machine_id: resolved.machine_id,
        });
        state.cached = Some(CachedEndpoint {
            snapshot: Arc::clone(&snapshot),
            credential,
            credential_revision: revision,
        });
        self.current_generation
            .store(snapshot.generation, Ordering::Release);
        self.prune_rejected_generations(snapshot.generation);
        Ok(snapshot)
    }

    fn cached_snapshot(&self, state: &EndpointState) -> Option<Arc<EndpointSnapshot>> {
        let cached = state.cached.as_ref()?;
        if cached.credential_revision != (self.credential_revision)() {
            return None;
        }
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
            credential_loader: Arc::new(|| super::credentials::load(/*auth_manager*/ None)),
            credential_revision: Arc::new(|| 0),
        }
    }
}

pub(super) fn shared_endpoint_manager(
    auth_manager: Option<Arc<AuthManager>>,
) -> Arc<CopilotEndpointManager> {
    let Some(auth_manager) = auth_manager else {
        static MANAGER: OnceLock<Arc<CopilotEndpointManager>> = OnceLock::new();
        return Arc::clone(MANAGER.get_or_init(|| Arc::new(CopilotEndpointManager::default())));
    };
    // A process can host multiple Codex homes or credential policies. Share refresh/rejection
    // state only among providers using the same resolved authentication manager.
    type ScopedManagers = Vec<(
        Weak<AuthManager>,
        PathBuf,
        Weak<CopilotEndpointManager>,
    )>;
    static MANAGERS: OnceLock<StdMutex<ScopedManagers>> = OnceLock::new();
    let mut managers = MANAGERS
        .get_or_init(StdMutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    managers.retain(|(_, _, manager)| manager.strong_count() != 0);
    let auth_scope = Arc::downgrade(&auth_manager);
    let credential_home = auth_manager.connection_credential_home();
    if let Some(manager) = managers
        .iter()
        .find(|(scope, home, _)| scope.ptr_eq(&auth_scope) && home == &credential_home)
        .and_then(|(_, _, manager)| manager.upgrade())
    {
        return manager;
    }
    let revision_manager = Arc::clone(&auth_manager);
    let manager = Arc::new(CopilotEndpointManager {
        credential_loader: Arc::new(move || super::credentials::load(Some(&auth_manager))),
        credential_revision: Arc::new(move || revision_manager.credential_revision()),
        ..CopilotEndpointManager::default()
    });
    managers.push((
        auth_scope,
        credential_home,
        Arc::downgrade(&manager),
    ));
    manager
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
