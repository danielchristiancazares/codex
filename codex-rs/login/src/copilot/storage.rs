use std::path::Path;
use std::sync::Arc;

use codex_keyring_store::KeyringStore;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use super::GitHubCopilotAuthError;
use super::GitHubCopilotCredential;

const CREDENTIAL_VERSION: u8 = 1;
const KEYRING_SERVICE: &str = "Codex GitHub Copilot";
const MAX_TOKEN_BYTES: usize = 16 * 1024;
const MACHINE_ID_HEX_BYTES: usize = 64;

#[derive(Clone)]
pub(super) struct CopilotCredentialStore {
    account: String,
    target: String,
    keyring: Arc<dyn KeyringStore>,
}

impl std::fmt::Debug for CopilotCredentialStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CopilotCredentialStore")
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize, Serialize)]
struct StoredCredential {
    version: u8,
    github_token: String,
    machine_id: String,
}

impl CopilotCredentialStore {
    pub(super) fn new_with_keyring(codex_home: &Path, keyring: Arc<dyn KeyringStore>) -> Self {
        let scope = codex_secrets::compute_keyring_account(codex_home);
        let account = format!("github-copilot|{scope}");
        let target = format!("codex/github-copilot/{scope}");
        Self {
            account,
            target,
            keyring,
        }
    }

    pub(super) fn load(&self) -> Result<Option<GitHubCopilotCredential>, GitHubCopilotAuthError> {
        #[cfg(windows)]
        let encoded = self
            .keyring
            .load_with_target(&self.target, KEYRING_SERVICE, &self.account);
        #[cfg(not(windows))]
        let encoded = self.keyring.load(KEYRING_SERVICE, &self.account);
        let Some(encoded) = encoded.map_err(|error| {
            GitHubCopilotAuthError::credential_store(format!(
                "load GitHub Copilot credential from the OS credential store: {error}"
            ))
        })?
        else {
            return Ok(None);
        };
        let stored = serde_json::from_str::<StoredCredential>(&encoded).map_err(|_error| {
            GitHubCopilotAuthError::malformed_credential(
                "decode GitHub Copilot credential from the OS credential store",
            )
        })?;
        validate_stored_credential(stored).map(Some)
    }

    pub(super) fn save(
        &self,
        credential: &GitHubCopilotCredential,
    ) -> Result<(), GitHubCopilotAuthError> {
        let encoded = serde_json::to_string(&StoredCredential {
            version: CREDENTIAL_VERSION,
            github_token: credential.token.clone(),
            machine_id: credential.machine_id.clone(),
        })
        .map_err(|error| {
            GitHubCopilotAuthError::persistence(format!(
                "encode GitHub Copilot credential: {error}"
            ))
        })?;
        #[cfg(windows)]
        let saved =
            self.keyring
                .save_with_target(&self.target, KEYRING_SERVICE, &self.account, &encoded);
        #[cfg(not(windows))]
        let saved = self.keyring.save(KEYRING_SERVICE, &self.account, &encoded);
        saved.map_err(|error| {
            GitHubCopilotAuthError::persistence(format!(
                "save GitHub Copilot credential in the OS credential store: {error}"
            ))
        })
    }

    pub(super) fn delete(&self) -> Result<bool, GitHubCopilotAuthError> {
        #[cfg(windows)]
        let deleted = self
            .keyring
            .delete_with_target(&self.target, KEYRING_SERVICE, &self.account);
        #[cfg(not(windows))]
        let deleted = self.keyring.delete(KEYRING_SERVICE, &self.account);
        deleted.map_err(|error| {
            GitHubCopilotAuthError::credential_store(format!(
                "delete GitHub Copilot credential from the OS credential store: {error}"
            ))
        })
    }
}

fn validate_stored_credential(
    stored: StoredCredential,
) -> Result<GitHubCopilotCredential, GitHubCopilotAuthError> {
    if stored.version != CREDENTIAL_VERSION {
        return Err(GitHubCopilotAuthError::malformed_credential(format!(
            "unsupported GitHub Copilot credential version {}",
            stored.version
        )));
    }
    let token = stored.github_token.trim().to_string();
    if token.is_empty() || token.len() > MAX_TOKEN_BYTES {
        return Err(GitHubCopilotAuthError::malformed_credential(
            "stored GitHub Copilot token has an invalid length",
        ));
    }
    let machine_id = stored.machine_id.trim().to_ascii_lowercase();
    if machine_id.len() != MACHINE_ID_HEX_BYTES
        || !machine_id.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GitHubCopilotAuthError::malformed_credential(
            "stored GitHub Copilot machine ID is invalid",
        ));
    }
    Ok(GitHubCopilotCredential { token, machine_id })
}

pub(super) fn new_machine_id() -> String {
    let digest = Sha256::digest(rand::random::<[u8; 32]>());
    format!("{digest:x}")
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
