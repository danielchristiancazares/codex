use codex_app_server_protocol::RateLimitReachedType;
use codex_app_server_protocol::RateLimitSnapshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceAccessState {
    Unknown,
    NotBlocked,
    Blocked(WorkspaceLimitBlockReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceLimitBlockReason {
    RateLimitReached,
    SpendControlReached,
    OwnerCreditsDepleted,
    MemberCreditsDepleted,
    OwnerUsageLimitReached,
    MemberUsageLimitReached,
}

impl WorkspaceAccessState {
    pub(crate) fn from_snapshot(snapshot: &RateLimitSnapshot) -> Self {
        if let Some(reached_type) = snapshot.rate_limit_reached_type {
            return Self::Blocked(WorkspaceLimitBlockReason::from(reached_type));
        }

        match snapshot.spend_control_reached {
            Some(true) => Self::Blocked(WorkspaceLimitBlockReason::SpendControlReached),
            Some(false) => Self::NotBlocked,
            None => Self::Unknown,
        }
    }

    pub(crate) fn merge_rolling(self, snapshot: &RateLimitSnapshot) -> Self {
        if snapshot.rate_limit_reached_type.is_some() {
            return Self::from_snapshot(snapshot);
        }

        if matches!(self, Self::Blocked(reason) if reason != WorkspaceLimitBlockReason::SpendControlReached)
        {
            return self;
        }

        match snapshot.spend_control_reached {
            Some(true) => Self::Blocked(WorkspaceLimitBlockReason::SpendControlReached),
            Some(false) => Self::NotBlocked,
            None => self,
        }
    }
}

impl WorkspaceLimitBlockReason {
    pub(crate) fn summary(self) -> &'static str {
        match self {
            Self::RateLimitReached => "Blocked - rate limit reached",
            Self::SpendControlReached => "Blocked - workspace spending limit reached",
            Self::OwnerCreditsDepleted | Self::MemberCreditsDepleted => {
                "Blocked - workspace credits depleted"
            }
            Self::OwnerUsageLimitReached | Self::MemberUsageLimitReached => {
                "Blocked - workspace usage limit reached"
            }
        }
    }

    pub(crate) fn guidance(self) -> &'static str {
        match self {
            Self::RateLimitReached => "Wait for the limit to reset before continuing.",
            Self::SpendControlReached => "Review workspace spending controls before continuing.",
            Self::OwnerCreditsDepleted => "Add credits to continue using Codex.",
            Self::MemberCreditsDepleted => "Ask a workspace owner to add credits.",
            Self::OwnerUsageLimitReached => "Increase the workspace limit to continue.",
            Self::MemberUsageLimitReached => "Ask a workspace owner to increase the limit.",
        }
    }
}

impl From<RateLimitReachedType> for WorkspaceLimitBlockReason {
    fn from(value: RateLimitReachedType) -> Self {
        match value {
            RateLimitReachedType::RateLimitReached => Self::RateLimitReached,
            RateLimitReachedType::WorkspaceOwnerCreditsDepleted => Self::OwnerCreditsDepleted,
            RateLimitReachedType::WorkspaceMemberCreditsDepleted => Self::MemberCreditsDepleted,
            RateLimitReachedType::WorkspaceOwnerUsageLimitReached => Self::OwnerUsageLimitReached,
            RateLimitReachedType::WorkspaceMemberUsageLimitReached => Self::MemberUsageLimitReached,
        }
    }
}

#[cfg(test)]
#[path = "rate_limit_block_tests.rs"]
mod tests;
