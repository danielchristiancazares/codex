use std::fmt;

use sha2::Digest;
use sha2::Sha256;

use super::credentials::CopilotCredential;

/// Digest of the routing and credentials actually installed in an endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CatalogIdentity([u8; 32]);

impl CatalogIdentity {
    pub(super) fn for_credential(credential: &CopilotCredential) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"codex-copilot-models-v1");
        for value in [
            credential.base_url.trim_end_matches('/'),
            credential.token.as_str(),
        ] {
            digest.update((value.len() as u64).to_le_bytes());
            digest.update(value.as_bytes());
        }
        if let Some(machine_id) = credential.machine_id.as_deref() {
            digest.update((machine_id.len() as u64).to_le_bytes());
            digest.update(machine_id.as_bytes());
        }
        Self(digest.finalize().into())
    }
}

impl fmt::Display for CatalogIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("copilot:")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Selects whether the models manager can reuse an authenticated catalog.
pub(super) enum CatalogCachePolicy {
    FetchAuthenticatedCatalog,
    ReuseAuthenticatedCatalog(CatalogIdentity),
}

// Adapt the operational policy to the upstream models-manager cache interface.
impl From<CatalogCachePolicy> for Option<String> {
    fn from(policy: CatalogCachePolicy) -> Self {
        match policy {
            CatalogCachePolicy::FetchAuthenticatedCatalog => None,
            CatalogCachePolicy::ReuseAuthenticatedCatalog(identity) => Some(identity.to_string()),
        }
    }
}
