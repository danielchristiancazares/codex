use serde::Deserialize;
use serde::Serialize;

/// Path-independent metadata for one retained execution-output stream.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ArtifactDescriptor {
    pub artifact_ref: String,
    pub state: ArtifactState,
    pub media_type: String,
    pub encoding: ArtifactEncoding,
    pub byte_count: u64,
    pub observed_byte_count: u64,
    pub line_count: Option<u64>,
    pub sha256: Option<String>,
    pub capture: ArtifactCapture,
    pub producer: ArtifactProducer,
    pub environment_id: String,
    pub retention: ArtifactRetention,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactState {
    Pending,
    Complete,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactEncoding {
    Utf8,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactCapture {
    Complete,
    Truncated,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ArtifactProducer {
    pub kind: ArtifactProducerKind,
    pub item_id: String,
    pub process_id: Option<String>,
    pub stream: ArtifactStream,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactProducerKind {
    ProcessStream,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ArtifactRetention {
    pub expires_at: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExecOutputArtifacts {
    pub stdout: ArtifactDescriptor,
    pub stderr: ArtifactDescriptor,
}
