use super::*;
use crate::NonEmptyString;

/// A single-use GitHub login ceremony with a UI-visible authorization challenge.
pub struct GitHubCopilotLogin {
    auth: GitHubCopilotAuth,
    device_code: device_flow::DeviceCode,
}

impl GitHubCopilotAuth {
    pub async fn begin_device_login(&self) -> Result<GitHubCopilotLogin, GitHubCopilotAuthError> {
        let device_code =
            device_flow::request_device_code(&self.http_client_factory, &self.oauth_endpoints)
                .await?;
        Ok(GitHubCopilotLogin {
            auth: self.clone(),
            device_code,
        })
    }
}

impl GitHubCopilotLogin {
    pub fn verification_url(&self) -> &str {
        &self.device_code.verification_uri
    }
    pub fn user_code(&self) -> &str {
        &self.device_code.user_code
    }

    pub async fn finish(self) -> Result<NonEmptyString, GitHubCopilotAuthError> {
        let token = device_flow::complete_device_authorization(
            &self.auth.http_client_factory,
            &self.auth.oauth_endpoints,
            self.device_code,
        )
        .await?;
        let name =
            NonEmptyString::new(self.auth.account_for_token(&token).await?).map_err(|_| {
                GitHubCopilotAuthError::account("GitHub returned an invalid account name.")
            })?;
        self.auth.credential_store.save(&GitHubCopilotCredential {
            token,
            machine_id: storage::new_machine_id(),
        })?;
        *self
            .auth
            .credential_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CredentialState::Unloaded;
        Ok(name)
    }
}

#[cfg(test)]
#[path = "saved_login_tests.rs"]
mod tests;
