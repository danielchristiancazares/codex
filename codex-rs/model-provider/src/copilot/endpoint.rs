use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::ACCEPT;
use http::header::AUTHORIZATION;
use http::header::USER_AGENT;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio::time::timeout;

use super::credentials::CopilotCredential;
use super::credentials::CredentialLoadError;

const COPILOT_RPC_TIMEOUT: Duration = Duration::from_secs(30);
const COPILOT_TOKEN_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_TOKEN_REFRESH: Duration = Duration::from_secs(180);
const MIN_TOKEN_REFRESH: Duration = Duration::from_secs(5);
const MAX_ERROR_BODY_BYTES: usize = 256;

type CredentialLoader =
    Arc<dyn Fn() -> Result<Option<CopilotCredential>, CredentialLoadError> + Send + Sync + 'static>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EndpointSource {
    Direct,
    Cli,
}

/// Immutable endpoint material obtained from direct GitHub authentication or the Copilot CLI.
#[derive(Clone)]
pub(super) struct EndpointSnapshot {
    pub(super) generation: u64,
    pub(super) base_url: String,
    pub(super) headers: HeaderMap,
    pub(super) bound_model: Option<String>,
    pub(super) source: EndpointSource,
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
            .field("source", &self.source)
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
    Direct,
    CliModelsBroker,
    CliInference { thread_id: ThreadId, model: String },
}

#[derive(Clone, Debug)]
enum EndpointRequest {
    ModelsBroker,
    Inference { thread_id: ThreadId, model: String },
}

/// Resolves direct credentials when configured and otherwise preserves the installed CLI session
/// contract. Direct endpoints share an account-level cache; CLI inference remains thread/model
/// scoped because its session token may be bound to both.
pub(super) struct CopilotEndpointManager {
    state: Mutex<EndpointState>,
    rejected_generations: StdMutex<HashSet<u64>>,
    credential_loader: CredentialLoader,
    http_client_factory: HttpClientFactory,
}

impl fmt::Debug for CopilotEndpointManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CopilotEndpointManager")
            .field("state", &self.state)
            .field("rejected_generations", &self.rejected_generations)
            .field("http_client_factory", &self.http_client_factory)
            .finish_non_exhaustive()
    }
}

impl CopilotEndpointManager {
    pub(super) async fn endpoint(&self) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        self.endpoint_for_request(EndpointRequest::ModelsBroker)
            .await
    }

    pub(super) async fn endpoint_for_model(
        &self,
        thread_id: ThreadId,
        model: &str,
    ) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        let endpoint = self
            .endpoint_for_request(EndpointRequest::Inference {
                thread_id,
                model: model.to_string(),
            })
            .await?;
        if let Some(bound_model) = endpoint.bound_model.as_deref()
            && bound_model != model
        {
            return Err(CodexErr::Fatal(format!(
                "Copilot endpoint bound model `{bound_model}` does not match requested model `{model}`"
            )));
        }
        Ok(endpoint)
    }

    async fn endpoint_for_request(
        &self,
        request: EndpointRequest,
    ) -> codex_protocol::error::Result<Arc<EndpointSnapshot>> {
        let credential = self.load_credential().await.map_err(CodexErr::Fatal)?;
        let purpose = match (&credential, &request) {
            (Some(_), _) => EndpointPurpose::Direct,
            (None, EndpointRequest::ModelsBroker) => EndpointPurpose::CliModelsBroker,
            (None, EndpointRequest::Inference { thread_id, model }) => {
                EndpointPurpose::CliInference {
                    thread_id: *thread_id,
                    model: model.clone(),
                }
            }
        };

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
                .get_or_init(|| self.resolve_endpoint(&purpose, credential.clone()))
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
                            source: resolved.source,
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

    async fn load_credential(&self) -> Result<Option<CopilotCredential>, String> {
        let credential_loader = Arc::clone(&self.credential_loader);
        tokio::task::spawn_blocking(move || credential_loader())
            .await
            .map_err(|error| format!("load GitHub credential for Copilot: {error}"))?
            .map_err(|error| error.to_string())
    }

    async fn resolve_endpoint(
        &self,
        purpose: &EndpointPurpose,
        credential: Option<CopilotCredential>,
    ) -> Result<ResolvedEndpoint, String> {
        match (purpose, credential) {
            (EndpointPurpose::Direct, Some(credential)) => {
                self.resolve_direct_endpoint(credential).await
            }
            (EndpointPurpose::CliModelsBroker, None) => timeout(
                COPILOT_RPC_TIMEOUT,
                super::native_session::resolve_models_endpoint(),
            )
            .await
            .map_err(|_| "Copilot CLI endpoint request timed out".to_string())?,
            (EndpointPurpose::CliInference { thread_id, model }, None) => timeout(
                COPILOT_RPC_TIMEOUT,
                super::native_session::resolve_endpoint(*thread_id, model),
            )
            .await
            .map_err(|_| "Copilot CLI endpoint request timed out".to_string())?,
            (EndpointPurpose::Direct, None) => {
                Err("direct Copilot endpoint resolution requires a credential".to_string())
            }
            (EndpointPurpose::CliModelsBroker | EndpointPurpose::CliInference { .. }, Some(_)) => {
                Err("Copilot CLI endpoint resolution received a direct credential".to_string())
            }
        }
    }

    async fn resolve_direct_endpoint(
        &self,
        credential: CopilotCredential,
    ) -> Result<ResolvedEndpoint, String> {
        let (token, token_url) = match credential {
            CopilotCredential::ApiToken { token, api_url } => {
                return resolved_direct_endpoint(token, api_url, DEFAULT_TOKEN_REFRESH);
            }
            CopilotCredential::GitHubToken { token, token_url } => (token, token_url),
        };
        let client = self
            .http_client_factory
            .build_client_without_request_logging(&token_url, ClientRouteClass::Auth)
            .map_err(|error| format!("build Copilot token client: {error}"))?;
        let headers = token_exchange_headers(&token)?;
        let exchange = async {
            let response = client
                .get(&token_url)
                .headers(headers)
                .send()
                .await
                .map_err(|error| format!("request Copilot API token: {error}"))?;
            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(format!(
                    "Copilot API token request returned {status}: {}",
                    truncate(&body)
                ));
            }
            response
                .json::<CopilotTokenResponse>()
                .await
                .map_err(|error| format!("decode Copilot API token: {error}"))
        };
        let response = timeout(COPILOT_TOKEN_TIMEOUT, exchange)
            .await
            .map_err(|_| "Copilot API token request timed out".to_string())??;
        if response.token.is_empty() {
            return Err("Copilot API token response did not include a token".to_string());
        }

        let base_url = response
            .endpoints
            .and_then(|endpoints| endpoints.api)
            .ok_or_else(|| {
                "Copilot API token response did not include `endpoints.api`".to_string()
            })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        resolved_direct_endpoint(
            response.token,
            base_url,
            refresh_after(response.expires_at, i64::try_from(now).unwrap_or(i64::MAX)),
        )
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
            credential_loader: Arc::new(super::credentials::load),
            http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
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
    pub(super) source: EndpointSource,
}

#[derive(Debug, Deserialize)]
struct CopilotTokenResponse {
    token: String,
    #[serde(default)]
    expires_at: Option<i64>,
    #[serde(default)]
    endpoints: Option<CopilotTokenEndpoints>,
}

#[derive(Debug, Deserialize)]
struct CopilotTokenEndpoints {
    #[serde(default)]
    api: Option<String>,
}

fn token_exchange_headers(github_token: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(super::identity::TOKEN_EXCHANGE_USER_AGENT),
    );
    headers.insert(
        HeaderName::from_static("editor-version"),
        HeaderValue::from_static(super::identity::TOKEN_EXCHANGE_EDITOR_VERSION),
    );
    headers.insert(
        HeaderName::from_static("editor-plugin-version"),
        HeaderValue::from_static(super::identity::TOKEN_EXCHANGE_EDITOR_PLUGIN_VERSION),
    );
    let mut authorization = HeaderValue::from_str(&format!("token {github_token}"))
        .map_err(|error| format!("decode GitHub credential for Copilot: {error}"))?;
    authorization.set_sensitive(true);
    headers.insert(AUTHORIZATION, authorization);
    Ok(headers)
}

fn resolved_direct_endpoint(
    token: String,
    base_url: String,
    refresh_after: Duration,
) -> Result<ResolvedEndpoint, String> {
    let mut headers = HeaderMap::new();
    let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
        .map_err(|error| format!("decode Copilot API token: {error}"))?;
    authorization.set_sensitive(true);
    headers.insert(AUTHORIZATION, authorization);
    Ok(ResolvedEndpoint {
        base_url: validate_api_url(&base_url)?,
        headers,
        bound_model: None,
        refresh_after,
        source: EndpointSource::Direct,
    })
}

fn validate_api_url(base_url: &str) -> Result<String, String> {
    let uri = base_url
        .parse::<http::Uri>()
        .map_err(|_| "Copilot token service returned an invalid API URL".to_string())?;
    let scheme = uri.scheme_str();
    let host = uri
        .host()
        .ok_or_else(|| "Copilot token service returned an invalid API URL".to_string())?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if scheme != Some("https") && !(scheme == Some("http") && loopback) {
        return Err("Copilot API URL must use HTTPS unless it targets loopback".to_string());
    }
    Ok(base_url.trim_end_matches('/').to_string())
}

fn refresh_after(expires_at: Option<i64>, now: i64) -> Duration {
    expires_at
        .map(|expires_at| {
            let remaining = expires_at.saturating_sub(now);
            if remaining <= 0 {
                return MIN_TOKEN_REFRESH;
            }
            Duration::from_secs(u64::try_from(remaining).unwrap_or(1).saturating_mul(4) / 5)
        })
        .unwrap_or(DEFAULT_TOKEN_REFRESH)
        .max(MIN_TOKEN_REFRESH)
}

fn truncate(value: &str) -> String {
    let mut end = value.len().min(MAX_ERROR_BODY_BYTES);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let suffix = if end < value.len() {
        "…"
    } else {
        Default::default()
    };
    format!("{}{suffix}", &value[..end])
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
