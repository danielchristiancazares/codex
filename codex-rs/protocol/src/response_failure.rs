//! Terminal inference failures and their accounting consequences.

use crate::protocol::TokenUsage;
use std::future::Future;
use thiserror::Error;

const DIAGNOSTIC_MAX_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct NonEmptyString(String);

impl NonEmptyString {
    fn parse(value: &str) -> Result<Self, RequiredResponseField> {
        if value.trim().is_empty() {
            return Err(RequiredResponseField);
        }
        Ok(Self(value.to_owned()))
    }
}

impl std::fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Error)]
#[error("response id and incomplete reason must be nonempty strings")]
pub struct RequiredResponseField;

/// Selects the accounting operation for a terminal response.
#[derive(Debug, Clone)]
pub enum TerminalResponseUsage {
    RecordServerUsage(TokenUsage),
    PreserveRecordedUsage,
}

impl TerminalResponseUsage {
    async fn record<'a, F, Fut>(&'a self, record: F)
    where
        F: FnOnce(&'a TokenUsage) -> Fut,
        Fut: Future,
    {
        match self {
            Self::RecordServerUsage(usage) => {
                let _ = record(usage).await;
            }
            Self::PreserveRecordedUsage => {}
        }
    }
}

#[derive(Debug, Clone, Error)]
#[error("incomplete response returned, reason: {reason} (response {response_id})")]
pub struct IncompleteResponse {
    response_id: NonEmptyString,
    reason: NonEmptyString,
    usage: TerminalResponseUsage,
}

impl IncompleteResponse {
    pub fn new(
        response_id: &str,
        reason: &str,
        usage: TerminalResponseUsage,
    ) -> Result<Self, RequiredResponseField> {
        Ok(Self {
            response_id: NonEmptyString::parse(response_id)?,
            reason: NonEmptyString::parse(reason)?,
            usage,
        })
    }

    /// Accounts reported usage once before the sampling loop returns this terminal failure.
    pub async fn record_usage<'a, F, Fut>(&'a self, record: F)
    where
        F: FnOnce(&'a TokenUsage) -> Fut,
        Fut: Future,
    {
        self.usage.record(record).await;
    }
}

/// Keeps bounded diagnostic evidence solely on the failed-event path.
#[derive(Debug, Clone, Error)]
#[error("response protocol error: {message}")]
pub struct ResponseProtocolFailure {
    message: NonEmptyString,
    raw_event: String,
}

impl ResponseProtocolFailure {
    pub fn for_event(kind: &str, detail: &str, raw: &str) -> Self {
        let raw_event = if raw.len() <= DIAGNOSTIC_MAX_BYTES {
            raw.to_owned()
        } else {
            let mut end = DIAGNOSTIC_MAX_BYTES - "…".len();
            while !raw.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &raw[..end])
        };
        Self {
            message: NonEmptyString(format!("invalid {kind} event: {detail}")),
            raw_event,
        }
    }

    pub fn diagnostic(&self) -> &str {
        &self.raw_event
    }
}
