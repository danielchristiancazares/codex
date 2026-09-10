//! Turn-owned cancellation authority for asynchronous hook work.

use std::borrow::Borrow;
use std::collections::HashMap;
use tokio::task::AbortHandle;

#[derive(Debug, Hash, Eq, PartialEq)]
pub(super) struct NonEmptyString(String);

impl Borrow<str> for NonEmptyString {
    fn borrow(&self) -> &str {
        &self.0
    }
}

pub(super) enum AsyncTaskOwner {
    Session,
    Turn(NonEmptyString),
}

#[derive(Debug)]
pub(super) struct TurnIdentityRequired;

impl std::fmt::Display for TurnIdentityRequired {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("turn-scoped async hook requires a nonempty turn id")
    }
}

impl std::error::Error for TurnIdentityRequired {}

impl AsyncTaskOwner {
    pub(super) fn for_turn(turn_id: &str) -> Result<Self, TurnIdentityRequired> {
        if turn_id.trim().is_empty() {
            return Err(TurnIdentityRequired);
        }
        Ok(Self::Turn(NonEmptyString(turn_id.to_owned())))
    }

    pub(super) fn retain_abort_handle(
        self,
        handles: &mut HashMap<NonEmptyString, Vec<AbortHandle>>,
        handle: AbortHandle,
    ) {
        match self {
            Self::Session => {}
            Self::Turn(turn_id) => handles.entry(turn_id).or_default().push(handle),
        }
    }
}
