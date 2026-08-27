use std::io;

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("invalid exec-output artifact reference")]
    InvalidReference,
    #[error("artifact was not found")]
    NotFound,
    #[error("artifact access is outside the owning thread or workspace authority")]
    Unauthorized,
    #[error("artifact content is not complete")]
    Incomplete,
    #[error("artifact finalization previously failed")]
    FinalizationFailed,
    #[error("artifact reservation is no longer pending")]
    InvalidState,
    #[error("artifact retention has expired")]
    Expired,
    #[error("artifact content or metadata is corrupt")]
    Corrupt,
    #[error("artifact query is invalid: {0}")]
    InvalidQuery(String),
    #[error("artifact storage quota exceeded")]
    QuotaExceeded,
    #[error("artifact storage failed: {0}")]
    Storage(#[from] io::Error),
    #[error("artifact metadata failed: {0}")]
    Metadata(#[from] serde_json::Error),
}
