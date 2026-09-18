//! A request's client-enforced window, frozen from its selected model.

use codex_protocol::openai_models::ModelInfo;

/// The enforcement boundary frozen from the model selected for a request.
#[derive(Clone, Copy, Debug)]
pub enum RequestBudget {
    ProviderEnforced,
    ClientEnforced(u64),
}

/// Evidence that an estimate fits the selected request budget.
#[derive(Debug)]
pub struct RequestFit;

#[derive(Debug, thiserror::Error)]
#[error(
    "request estimate of {estimated_tokens} tokens exceeds the {allowed_tokens}-token context window"
)]
pub struct RequestExceedsWindow {
    estimated_tokens: i64,
    allowed_tokens: u64,
}

impl RequestBudget {
    pub fn for_model(model: &ModelInfo) -> Self {
        match model.usable_context_window() {
            Some(window) => Self::ClientEnforced(u64::try_from(window.max(0)).unwrap_or(u64::MAX)),
            None => Self::ProviderEnforced,
        }
    }

    pub fn check(self, estimated_tokens: i64) -> Result<RequestFit, RequestExceedsWindow> {
        match self {
            Self::ProviderEnforced => Ok(RequestFit),
            Self::ClientEnforced(allowed_tokens) => {
                if u64::try_from(estimated_tokens.max(0)).unwrap_or(u64::MAX) <= allowed_tokens {
                    Ok(RequestFit)
                } else {
                    Err(RequestExceedsWindow {
                        estimated_tokens,
                        allowed_tokens,
                    })
                }
            }
        }
    }

    pub fn reduced_target(self, estimated_tokens: i64) -> i64 {
        let applicable = match self {
            Self::ProviderEnforced => estimated_tokens,
            Self::ClientEnforced(tokens) => {
                estimated_tokens.min(i64::try_from(tokens).unwrap_or(i64::MAX))
            }
        };
        applicable.max(0).saturating_mul(4) / 5
    }
}
