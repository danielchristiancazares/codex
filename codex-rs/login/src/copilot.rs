mod device_flow;
mod storage;

use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::read_response_body_bounded;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use http::HeaderMap;
use http::HeaderValue;
use http::header::ACCEPT;
use http::header::AUTHORIZATION;
use http::header::USER_AGENT;
use thiserror::Error;

use self::device_flow::OAuthEndpoints;
use self::storage::CopilotCredentialStore;

const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const GITHUB_REST_API_VERSION: &str = "2022-11-28";
const MAX_GITHUB_RESPONSE_BYTES: usize = 64 * 1024;
const GITHUB_ACCOUNT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// A GitHub OAuth token paired with the stable machine identity that first used it.
#[derive(Clone, Eq, PartialEq)]
pub struct GitHubCopilotCredential {
    token: String,
    machine_id: String,
}

impl GitHubCopilotCredential {
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn machine_id(&self) -> &str {
        &self.machine_id
    }
}

impl fmt::Debug for GitHubCopilotCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitHubCopilotCredential")
            .field("token", &"[REDACTED]")
            .field("machine_id", &self.machine_id)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitHubCopilotAuthErrorKind {
    Configuration,
    CredentialStore,
    MalformedCredential,
    OAuth,
    AccountValidation,
    AccountRejected,
    Persistence,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message}")]
pub struct GitHubCopilotAuthError {
    kind: GitHubCopilotAuthErrorKind,
    message: String,
}

impl GitHubCopilotAuthError {
    fn new(kind: GitHubCopilotAuthErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> GitHubCopilotAuthErrorKind {
        self.kind
    }

    fn oauth(message: impl Into<String>) -> Self {
        Self::new(GitHubCopilotAuthErrorKind::OAuth, message)
    }

    fn account(message: impl Into<String>) -> Self {
        Self::new(GitHubCopilotAuthErrorKind::AccountValidation, message)
    }

    fn account_rejected(message: impl Into<String>) -> Self {
        Self::new(GitHubCopilotAuthErrorKind::AccountRejected, message)
    }

    fn credential_store(message: impl Into<String>) -> Self {
        Self::new(GitHubCopilotAuthErrorKind::CredentialStore, message)
    }

    fn malformed_credential(message: impl Into<String>) -> Self {
        Self::new(GitHubCopilotAuthErrorKind::MalformedCredential, message)
    }

    fn persistence(message: impl Into<String>) -> Self {
        Self::new(GitHubCopilotAuthErrorKind::Persistence, message)
    }
}

/// In-process GitHub OAuth and credential lifecycle for the native Copilot provider.
#[derive(Clone)]
pub struct GitHubCopilotAuth {
    credential_store: CopilotCredentialStore,
    credential_state: Arc<Mutex<CredentialState>>,
    http_client_factory: HttpClientFactory,
    oauth_endpoints: OAuthEndpoints,
    github_api_base_url: String,
}

#[derive(Default)]
enum CredentialState {
    #[default]
    Unloaded,
    Loaded(Option<GitHubCopilotCredential>),
}

impl fmt::Debug for GitHubCopilotAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitHubCopilotAuth")
            .field("credential_store", &self.credential_store)
            .field("http_client_factory", &self.http_client_factory)
            .field("oauth_endpoints", &self.oauth_endpoints)
            .field("github_api_base_url", &self.github_api_base_url)
            .finish()
    }
}

impl GitHubCopilotAuth {
    pub fn new() -> Result<Self, GitHubCopilotAuthError> {
        let codex_home = codex_utils_home_dir::find_codex_home().map_err(|error| {
            GitHubCopilotAuthError::new(
                GitHubCopilotAuthErrorKind::Configuration,
                format!("resolve CODEX_HOME for GitHub Copilot authentication: {error}"),
            )
        })?;
        Ok(Self::new_in(
            codex_home.as_ref(),
            HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ))
    }

    pub fn new_in(codex_home: &Path, http_client_factory: HttpClientFactory) -> Self {
        Self::new_with_parts(
            codex_home,
            http_client_factory,
            std::sync::Arc::new(DefaultKeyringStore),
            OAuthEndpoints::github(),
            GITHUB_API_BASE_URL.to_string(),
        )
    }

    fn new_with_parts(
        codex_home: &Path,
        http_client_factory: HttpClientFactory,
        keyring: std::sync::Arc<dyn KeyringStore>,
        oauth_endpoints: OAuthEndpoints,
        github_api_base_url: String,
    ) -> Self {
        Self {
            credential_store: CopilotCredentialStore::new_with_keyring(codex_home, keyring),
            credential_state: Arc::new(Mutex::new(CredentialState::default())),
            http_client_factory,
            oauth_endpoints,
            github_api_base_url,
        }
    }

    /// Loads the credential paired with this Codex home's machine identity.
    pub fn credential(&self) -> Result<Option<GitHubCopilotCredential>, GitHubCopilotAuthError> {
        let mut state = self
            .credential_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*state {
            CredentialState::Loaded(credential) => Ok(credential.clone()),
            CredentialState::Unloaded => {
                let credential = self.credential_store.load()?;
                *state = CredentialState::Loaded(credential.clone());
                Ok(credential)
            }
        }
    }

    /// Runs GitHub's device authorization flow and returns the authenticated login.
    pub async fn login(&self, force: bool) -> Result<String, GitHubCopilotAuthError> {
        let existing = match self.credential() {
            Ok(existing) => existing,
            Err(error)
                if force && error.kind() == GitHubCopilotAuthErrorKind::MalformedCredential =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        if !force && let Some(existing) = &existing {
            return self
                .account_for_token(existing.token())
                .await
                .map_err(|error| {
                    if error.kind() == GitHubCopilotAuthErrorKind::AccountRejected {
                        GitHubCopilotAuthError::account_rejected(format!(
                            "stored GitHub credential was rejected; retry with \
                             `codex login --provider copilot --force`: {error}"
                        ))
                    } else {
                        error
                    }
                });
        }

        let device_code =
            device_flow::request_device_code(&self.http_client_factory, &self.oauth_endpoints)
                .await?;
        eprintln!(
            "Open {} and enter code {} to authorize GitHub Copilot.",
            device_code.verification_uri, device_code.user_code
        );
        let token = device_flow::complete_device_authorization(
            &self.http_client_factory,
            &self.oauth_endpoints,
            device_code,
        )
        .await?;
        let login = self.account_for_token(&token).await?;
        let credential = GitHubCopilotCredential {
            token,
            machine_id: existing
                .map(|credential| credential.machine_id)
                .unwrap_or_else(storage::new_machine_id),
        };
        *self
            .credential_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            CredentialState::Loaded(Some(credential.clone()));
        if let Err(error) = self.credential_store.save(&credential) {
            return Err(GitHubCopilotAuthError::persistence(format!(
                "GitHub authentication succeeded for {login}, but the credential was not \
                 persisted and is available only to this process: {error}"
            )));
        }
        Ok(login)
    }

    pub async fn account(&self) -> Result<Option<String>, GitHubCopilotAuthError> {
        let Some(credential) = self.credential()? else {
            return Ok(None);
        };
        self.account_for_token(credential.token()).await.map(Some)
    }

    pub fn logout(&self) -> Result<bool, GitHubCopilotAuthError> {
        let cached = std::mem::replace(
            &mut *self
                .credential_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            CredentialState::Loaded(None),
        );
        self.credential_store
            .delete()
            .map(|deleted| deleted || matches!(cached, CredentialState::Loaded(Some(_))))
    }

    async fn account_for_token(&self, token: &str) -> Result<String, GitHubCopilotAuthError> {
        let url = format!("{}/user", self.github_api_base_url.trim_end_matches('/'));
        let client = self
            .http_client_factory
            .build_client_without_redirects_or_request_logging(&url, ClientRouteClass::Auth)
            .map_err(|error| {
                GitHubCopilotAuthError::account(format!("build GitHub account client: {error}"))
            })?;
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(device_flow::COPILOT_USER_AGENT),
        );
        headers.insert(
            "x-github-api-version",
            HeaderValue::from_static(GITHUB_REST_API_VERSION),
        );
        let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| GitHubCopilotAuthError::account("encode GitHub account credential"))?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
        let response = tokio::time::timeout(
            GITHUB_ACCOUNT_TIMEOUT,
            client.get(&url).headers(headers).send(),
        )
        .await
        .map_err(|_| GitHubCopilotAuthError::account("GitHub account request timed out"))?
        .map_err(|_| GitHubCopilotAuthError::account("request GitHub account"))?;
        let status = response.status();
        let body = tokio::time::timeout(
            GITHUB_ACCOUNT_TIMEOUT,
            read_response_body_bounded(response, MAX_GITHUB_RESPONSE_BYTES),
        )
        .await
        .map_err(|_| GitHubCopilotAuthError::account("GitHub account response timed out"))?
        .map_err(|error| {
            GitHubCopilotAuthError::account(format!("read GitHub account response: {error}"))
        })?;
        if matches!(
            status,
            http::StatusCode::UNAUTHORIZED | http::StatusCode::FORBIDDEN
        ) {
            return Err(GitHubCopilotAuthError::account_rejected(format!(
                "GitHub account request returned {status}"
            )));
        }
        if !status.is_success() {
            return Err(GitHubCopilotAuthError::account(format!(
                "GitHub account request returned {status}"
            )));
        }
        let response = serde_json::from_slice::<GitHubUser>(&body)
            .map_err(|_| GitHubCopilotAuthError::account("decode GitHub account response"))?;
        if response.login.trim().is_empty() {
            return Err(GitHubCopilotAuthError::account(
                "GitHub account response did not include a login",
            ));
        }
        Ok(response.login)
    }
}

#[derive(serde::Deserialize)]
struct GitHubUser {
    login: String,
}

#[cfg(test)]
#[path = "copilot_tests.rs"]
mod tests;
