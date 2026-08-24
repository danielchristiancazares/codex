use std::fmt;

use codex_login::GitHubCopilotAuth;

const COPILOT_API_TOKEN_ENV: &str = "GITHUB_COPILOT_API_TOKEN";
const COPILOT_API_URL_ENV: &str = "COPILOT_API_URL";
const COPILOT_GITHUB_TOKEN_ENV: &str = "COPILOT_GITHUB_TOKEN";
const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";
const GH_TOKEN_ENV: &str = "GH_TOKEN";
const COPILOT_API_URL: &str = "https://api.githubcopilot.com";

#[derive(Clone, Eq, PartialEq)]
pub(super) struct CopilotCredential {
    pub(super) token: String,
    pub(super) base_url: String,
    pub(super) machine_id: Option<String>,
    pub(super) source: CopilotCredentialSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CopilotCredentialSource {
    ApiTokenEnvironment,
    DedicatedGitHubEnvironment,
    StoredOAuth,
    GhTokenEnvironment,
    GitHubTokenEnvironment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CredentialLoadError {
    message: String,
}

impl CredentialLoadError {
    pub(super) fn credential_store(message: String) -> Self {
        Self { message }
    }

    pub(super) fn missing() -> Self {
        Self {
            message: "GitHub Copilot native authentication requires \
                      `GITHUB_COPILOT_API_TOKEN`, `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, \
                      `GITHUB_TOKEN`, or `codex login --provider copilot`"
                .to_string(),
        }
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
        formatter
            .debug_struct("CopilotCredential")
            .field("token", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("machine_id", &self.machine_id)
            .field("source", &self.source)
            .finish()
    }
}

/// Loads the native credential used for direct Copilot API authentication.
pub(super) fn load() -> Result<CopilotCredential, CredentialLoadError> {
    load_with(environment_token, || {
        GitHubCopilotAuth::new()
            .map_err(|error| CredentialLoadError::credential_store(error.to_string()))?
            .credential()
            .map(|credential| {
                credential.map(|credential| {
                    (
                        credential.token().to_string(),
                        credential.machine_id().to_string(),
                    )
                })
            })
            .map_err(|error| CredentialLoadError::credential_store(error.to_string()))
    })
}

fn load_with<E, K>(environment: E, stored: K) -> Result<CopilotCredential, CredentialLoadError>
where
    E: Fn(&str) -> Option<String>,
    K: FnOnce() -> Result<Option<(String, String)>, CredentialLoadError>,
{
    let environment_value = |name| {
        environment(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    if let Some(token) = environment_value(COPILOT_API_TOKEN_ENV) {
        return Ok(CopilotCredential {
            token,
            base_url: environment_value(COPILOT_API_URL_ENV)
                .unwrap_or_else(|| COPILOT_API_URL.to_string()),
            machine_id: None,
            source: CopilotCredentialSource::ApiTokenEnvironment,
        });
    }
    if let Some(token) = environment_value(COPILOT_GITHUB_TOKEN_ENV) {
        return Ok(CopilotCredential {
            token,
            base_url: COPILOT_API_URL.to_string(),
            machine_id: None,
            source: CopilotCredentialSource::DedicatedGitHubEnvironment,
        });
    }
    if let Some((token, machine_id)) = stored()? {
        return Ok(CopilotCredential {
            token,
            base_url: COPILOT_API_URL.to_string(),
            machine_id: Some(machine_id),
            source: CopilotCredentialSource::StoredOAuth,
        });
    }
    for (name, source) in [
        (GH_TOKEN_ENV, CopilotCredentialSource::GhTokenEnvironment),
        (
            GITHUB_TOKEN_ENV,
            CopilotCredentialSource::GitHubTokenEnvironment,
        ),
    ] {
        if let Some(token) = environment_value(name) {
            return Ok(CopilotCredential {
                token,
                base_url: COPILOT_API_URL.to_string(),
                machine_id: None,
                source,
            });
        }
    }
    Err(CredentialLoadError::missing())
}

fn environment_token(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
