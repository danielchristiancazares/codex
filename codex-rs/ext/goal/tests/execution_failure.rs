#![allow(dead_code)]

#[path = "../src/accounting.rs"]
mod accounting;

use accounting::GoalAccountingState;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolName;
use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::TokenUsage;
use pretty_assertions::assert_eq;

const HOST_FAILURE: ToolCallOutcome = ToolCallOutcome::Failed {
    handler_executed: true,
};

#[test]
fn each_failure_turn_is_counted_once() {
    let state = GoalAccountingState::default();
    for turn in 1..=3 {
        let turn_id = format!("turn-{turn}");
        state.start_turn(&turn_id, ModeKind::Default, &TokenUsage::default());
        state.mark_turn_goal_active(&turn_id, "goal");
        state.record_tool_outcome(&turn_id, &ToolName::plain("exec"), HOST_FAILURE);
        assert_eq!(
            state.execution_failure_goal(&turn_id),
            (turn == 3).then(|| "goal".to_string())
        );
        state.record_tool_outcome(&turn_id, &ToolName::plain("exec"), HOST_FAILURE);
        assert_eq!(state.execution_failure_goal(&turn_id), None);
        state.finish_turn(&turn_id);
    }
}

#[test]
fn stale_turn_success_cannot_clear_current_goal_failures() {
    let state = GoalAccountingState::default();
    for turn in ["first", "second"] {
        state.start_turn(turn, ModeKind::Default, &TokenUsage::default());
        state.mark_turn_goal_active(turn, "goal");
        state.record_tool_outcome(turn, &ToolName::plain("exec"), HOST_FAILURE);
        assert_eq!(state.execution_failure_goal(turn), None);
    }
    state.start_turn("current", ModeKind::Default, &TokenUsage::default());
    state.mark_turn_goal_active("current", "goal");
    state.record_tool_outcome(
        "first",
        &ToolName::plain("shell"),
        ToolCallOutcome::Completed { success: true },
    );
    state.record_tool_outcome("current", &ToolName::plain("exec"), HOST_FAILURE);
    assert_eq!(state.execution_failure_goal("first"), None);
    assert_eq!(
        state.execution_failure_goal("current"),
        Some("goal".to_string())
    );
}

#[test]
fn only_executed_default_namespace_exec_failures_qualify() {
    for (mode, tool, outcome) in [
        (ModeKind::Plan, ToolName::plain("exec"), HOST_FAILURE),
        (
            ModeKind::Default,
            ToolName::new(Some("external".to_string()), "exec"),
            HOST_FAILURE,
        ),
        (
            ModeKind::Default,
            ToolName::plain("exec"),
            ToolCallOutcome::Blocked,
        ),
        (
            ModeKind::Default,
            ToolName::plain("exec"),
            ToolCallOutcome::Aborted,
        ),
    ] {
        let state = GoalAccountingState::default();
        for turn in ["first", "second", "third"] {
            state.start_turn(turn, mode, &TokenUsage::default());
            state.mark_turn_goal_active(turn, "goal");
            state.record_tool_outcome(turn, &tool, outcome);
            assert_eq!(state.execution_failure_goal(turn), None);
            state.finish_turn(turn);
        }
    }
}

#[test]
fn recovery_dominates_later_failures_without_clearing_continuation_errors() {
    let state = GoalAccountingState::default();
    state.start_turn("turn", ModeKind::Default, &TokenUsage::default());
    state.mark_turn_goal_active("turn", "goal");
    state.mark_continuation_failure("storage unavailable".to_string());
    state.record_tool_outcome(
        "turn",
        &ToolName::plain("shell"),
        ToolCallOutcome::Completed { success: true },
    );
    state.record_tool_outcome("turn", &ToolName::plain("exec"), HOST_FAILURE);
    assert_eq!(state.execution_failure_goal("turn"), None);
    state.clear_active_goal();
    assert_eq!(
        state.continuation_failure(),
        Some("storage unavailable".to_string())
    );
}
