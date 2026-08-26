use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAccess {
    pub thread_id: String,
    pub environment_id: String,
    pub workspace_authority: String,
}

impl ArtifactAccess {
    pub fn new(
        thread_id: impl Into<String>,
        environment_id: impl Into<String>,
        workspace_roots: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Self {
        let mut roots = workspace_roots
            .into_iter()
            .map(|root| root.as_ref().to_string())
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        let mut hasher = Sha256::new();
        for root in roots {
            hasher.update(root.as_bytes());
            hasher.update([0]);
        }
        Self {
            thread_id: thread_id.into(),
            environment_id: environment_id.into(),
            workspace_authority: format!("{:x}", hasher.finalize()),
        }
    }
}
