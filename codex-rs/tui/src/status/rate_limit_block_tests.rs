use super::*;
use pretty_assertions::assert_eq;

fn snapshot(
    rate_limit_reached_type: Option<RateLimitReachedType>,
    spend_control_reached: Option<bool>,
) -> RateLimitSnapshot {
    RateLimitSnapshot {
        limit_id: Some("codex".to_string()),
        limit_name: None,
        normal_model_slug: None,
        primary: None,
        secondary: None,
        credits: None,
        individual_limit: None,
        spend_control_reached,
        plan_type: None,
        rate_limit_reached_type,
    }
}

#[test]
fn snapshot_mapping_preserves_exhaustive_block_reasons() {
    let cases = [
        (
            Some(RateLimitReachedType::RateLimitReached),
            WorkspaceLimitBlockReason::RateLimitReached,
        ),
        (
            Some(RateLimitReachedType::WorkspaceOwnerCreditsDepleted),
            WorkspaceLimitBlockReason::OwnerCreditsDepleted,
        ),
        (
            Some(RateLimitReachedType::WorkspaceMemberCreditsDepleted),
            WorkspaceLimitBlockReason::MemberCreditsDepleted,
        ),
        (
            Some(RateLimitReachedType::WorkspaceOwnerUsageLimitReached),
            WorkspaceLimitBlockReason::OwnerUsageLimitReached,
        ),
        (
            Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
            WorkspaceLimitBlockReason::MemberUsageLimitReached,
        ),
    ];

    for (reached_type, expected) in cases {
        assert_eq!(
            WorkspaceAccessState::from_snapshot(&snapshot(reached_type, Some(true))),
            WorkspaceAccessState::Blocked(expected),
        );
    }
    assert_eq!(
        WorkspaceAccessState::from_snapshot(&snapshot(None, Some(true))),
        WorkspaceAccessState::Blocked(WorkspaceLimitBlockReason::SpendControlReached),
    );
    assert_eq!(
        WorkspaceAccessState::from_snapshot(&snapshot(None, Some(false))),
        WorkspaceAccessState::NotBlocked,
    );
    assert_eq!(
        WorkspaceAccessState::from_snapshot(&snapshot(None, None)),
        WorkspaceAccessState::Unknown,
    );
}

#[test]
fn rolling_spend_updates_do_not_replace_a_known_rate_limit_reason() {
    let blocked = WorkspaceAccessState::Blocked(WorkspaceLimitBlockReason::MemberUsageLimitReached);

    assert_eq!(blocked.merge_rolling(&snapshot(None, Some(true))), blocked,);
    assert_eq!(blocked.merge_rolling(&snapshot(None, Some(false))), blocked,);
    assert_eq!(blocked.merge_rolling(&snapshot(None, None)), blocked);
}

#[test]
fn rolling_false_clears_only_generic_spend_control_state() {
    assert_eq!(
        WorkspaceAccessState::Blocked(WorkspaceLimitBlockReason::SpendControlReached)
            .merge_rolling(&snapshot(None, Some(false))),
        WorkspaceAccessState::NotBlocked,
    );
}

#[test]
fn block_reason_copy_is_owner_and_member_specific() {
    let cases = [
        (
            WorkspaceLimitBlockReason::RateLimitReached,
            "Blocked - rate limit reached",
            "Wait for the limit to reset before continuing.",
        ),
        (
            WorkspaceLimitBlockReason::SpendControlReached,
            "Blocked - workspace spending limit reached",
            "Review workspace spending controls before continuing.",
        ),
        (
            WorkspaceLimitBlockReason::OwnerCreditsDepleted,
            "Blocked - workspace credits depleted",
            "Add credits to continue using Codex.",
        ),
        (
            WorkspaceLimitBlockReason::MemberCreditsDepleted,
            "Blocked - workspace credits depleted",
            "Ask a workspace owner to add credits.",
        ),
        (
            WorkspaceLimitBlockReason::OwnerUsageLimitReached,
            "Blocked - workspace usage limit reached",
            "Increase the workspace limit to continue.",
        ),
        (
            WorkspaceLimitBlockReason::MemberUsageLimitReached,
            "Blocked - workspace usage limit reached",
            "Ask a workspace owner to increase the limit.",
        ),
    ];

    for (reason, summary, guidance) in cases {
        assert_eq!((reason.summary(), reason.guidance()), (summary, guidance));
    }
}
