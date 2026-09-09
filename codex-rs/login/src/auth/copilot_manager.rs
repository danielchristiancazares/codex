use super::AuthManager;
use crate::GitHubCopilotAuth;

impl AuthManager {
    /// Creates Copilot authentication using this runtime's resolved home and storage policy.
    /// A fresh instance lets unauthorized recovery observe credentials saved by another process.
    pub fn copilot_auth(&self) -> GitHubCopilotAuth {
        GitHubCopilotAuth::new_in(
            &self.connection_credential_home(),
            self.auth_route_config.http_client_factory().clone(),
            self.auth_credentials_store_mode,
        )
    }
}
