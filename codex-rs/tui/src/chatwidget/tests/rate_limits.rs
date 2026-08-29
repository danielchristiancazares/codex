use super::*;
use crate::status::WorkspaceAccessState;
use crate::status::WorkspaceLimitBlockReason;
use codex_app_server_protocol::RateLimitReachedType;
use codex_app_server_protocol::RateLimitSnapshot;
use pretty_assertions::assert_eq;

fn snapshot(
    rate_limit_reached_type: Option<RateLimitReachedType>,
    spend_control_reached: Option<bool>,
) -> RateLimitSnapshot {
    RateLimitSnapshot {
        limit_id: Some("codex".to_string()),
        limit_name: None,
        primary: None,
        secondary: None,
        credits: None,
        individual_limit: None,
        spend_control_reached,
        plan_type: None,
        rate_limit_reached_type,
    }
}

#[tokio::test]
async fn sparse_rolling_snapshot_preserves_authoritative_workspace_block() {
    let (mut chat, _event_rx, _op_rx) =
        helpers::make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_rate_limit_snapshot(Some(snapshot(
        Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
        Some(true),
    )));

    chat.on_rolling_rate_limit_snapshot(snapshot(None, None));

    assert_eq!(
        chat.rate_limit_snapshots_by_limit_id["codex"].workspace_access,
        WorkspaceAccessState::Blocked(WorkspaceLimitBlockReason::MemberUsageLimitReached),
    );
}

#[tokio::test]
async fn later_authoritative_snapshot_clears_workspace_block() {
    let (mut chat, _event_rx, _op_rx) =
        helpers::make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_rate_limit_snapshot(Some(snapshot(
        Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached),
        Some(true),
    )));

    chat.on_rate_limit_snapshot(Some(snapshot(None, None)));

    assert_eq!(
        chat.rate_limit_snapshots_by_limit_id["codex"].workspace_access,
        WorkspaceAccessState::Unknown,
    );
}
