use crate::oauth::StoredOAuthTokens;

/// Stored OAuth credential state for an initialized MCP client.
#[derive(Clone, Debug, PartialEq)]
pub enum ManagedOAuthCredentials {
    /// The client has no managed OAuth runtime and does not own stored credentials.
    Unmanaged,
    /// The client manages OAuth credentials, but none are currently stored.
    Missing,
    /// The client manages the supplied stored credentials.
    Stored(StoredOAuthTokens),
}
