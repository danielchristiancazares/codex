use std::fmt;

use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;

const COPILOT_API_TOKEN_ENV: &str = "GITHUB_COPILOT_API_TOKEN";
const COPILOT_API_URL_ENV: &str = "COPILOT_API_URL";
const COPILOT_GITHUB_TOKEN_ENV: &str = "COPILOT_GITHUB_TOKEN";
const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";
const GH_TOKEN_ENV: &str = "GH_TOKEN";
const COPILOT_API_URL: &str = "https://api.githubcopilot.com";
const COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";
const KEYRING_SERVICE: &str = "Codex";
const KEYRING_ACCOUNT: &str = "github-copilot";
// This target stores the long-lived GitHub credential. Short-lived CAPI tokens stay in memory.
#[cfg(windows)]
const WINDOWS_CREDENTIAL_TARGET: &str = "codex/github-copilot";

#[derive(Clone, Eq, PartialEq)]
pub(super) enum CopilotCredential {
    ApiToken { token: String, api_url: String },
    GitHubToken { token: String, token_url: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CredentialLoadError {
    message: String,
}

impl CredentialLoadError {
    pub(super) fn credential_store(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for CredentialLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for CredentialLoadError {}

impl fmt::Debug for CopilotCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiToken { api_url, .. } => formatter
                .debug_struct("ApiToken")
                .field("token", &"[REDACTED]")
                .field("api_url", api_url)
                .finish(),
            Self::GitHubToken { token_url, .. } => formatter
                .debug_struct("GitHubToken")
                .field("token", &"[REDACTED]")
                .field("token_url", token_url)
                .finish(),
        }
    }
}

/// Returns `Ok(None)` only when no direct credential exists, allowing the caller to use the
/// installed Copilot CLI session. Credential-store failures remain distinct from absence.
pub(super) fn load() -> Result<Option<CopilotCredential>, CredentialLoadError> {
    load_with(environment_token, token_from_keyring)
}

fn load_with<E, K>(
    environment: E,
    keyring: K,
) -> Result<Option<CopilotCredential>, CredentialLoadError>
where
    E: Fn(&str) -> Option<String>,
    K: FnOnce() -> Result<Option<String>, CredentialLoadError>,
{
    if let Some(token) = environment(COPILOT_API_TOKEN_ENV) {
        return Ok(Some(CopilotCredential::ApiToken {
            token,
            api_url: environment(COPILOT_API_URL_ENV)
                .unwrap_or_else(|| COPILOT_API_URL.to_string()),
        }));
    }
    if let Some(token) = environment(COPILOT_GITHUB_TOKEN_ENV) {
        return Ok(Some(CopilotCredential::GitHubToken {
            token,
            token_url: COPILOT_TOKEN_URL.to_string(),
        }));
    }
    if let Some(token) = keyring()? {
        return Ok(Some(CopilotCredential::GitHubToken {
            token,
            token_url: COPILOT_TOKEN_URL.to_string(),
        }));
    }
    Ok([GH_TOKEN_ENV, GITHUB_TOKEN_ENV]
        .iter()
        .find_map(|name| environment(name))
        .map(|token| CopilotCredential::GitHubToken {
            token,
            token_url: COPILOT_TOKEN_URL.to_string(),
        }))
}

fn environment_token(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn token_from_keyring() -> Result<Option<String>, CredentialLoadError> {
    #[cfg(windows)]
    let token = DefaultKeyringStore
        .load_with_target(WINDOWS_CREDENTIAL_TARGET, KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|error| {
            CredentialLoadError::credential_store(format!(
                "load GitHub token from Windows Credential Manager target \
                 `{WINDOWS_CREDENTIAL_TARGET}`: {error}"
            ))
        })?;
    #[cfg(not(windows))]
    let token = DefaultKeyringStore
        .load(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|error| {
            CredentialLoadError::credential_store(format!(
                "load GitHub token from the OS credential store: {error}"
            ))
        })?;
    Ok(token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty()))
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
