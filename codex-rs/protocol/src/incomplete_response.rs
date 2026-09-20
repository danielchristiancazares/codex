//! Terminal Responses results that stopped before producing a complete answer.

use crate::protocol::TokenUsage;
use thiserror::Error;

/// A provider-declared incomplete response, including any reported usage.
///
/// This terminal event can include work already performed by the provider. Keep
/// it distinct from transport failures to avoid resampling as connection recovery.
#[derive(Debug, Error)]
#[error("Incomplete response returned, reason: {reason}")]
pub struct IncompleteResponse {
    /// Provider reason, or `unknown` when it was not supplied.
    pub reason: String,
    /// Provider response identity, when supplied, for durable usage attribution.
    pub response_id: Option<String>,
    /// Reported usage, including reasoning performed before the response stopped.
    pub token_usage: Option<TokenUsage>,
}
