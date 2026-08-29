//! Goal-scoped execution availability evidence, independent of continuation failures.

use super::GoalAccountingState;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolName;

#[derive(Debug, Default)]
pub(super) enum GoalExecutionStreak {
    #[default]
    Clear,
    Tracking {
        goal_id: String,
        qualifying_turns: u8,
    },
}

impl GoalExecutionStreak {
    pub(super) fn align_goal(&mut self, current_goal_id: &str) {
        if let Self::Tracking { goal_id, .. } = self
            && goal_id != current_goal_id
        {
            *self = Self::Clear;
        }
    }
}

#[derive(Debug, Default)]
pub(super) enum TurnExecutionEvidence {
    #[default]
    NoSignal,
    Unavailable,
    /// A successful tool dominates any failures in the same turn.
    Recovered,
    /// This turn's failure has already advanced the goal-scoped streak.
    Counted,
}

impl GoalAccountingState {
    pub(crate) fn record_tool_outcome(
        &self,
        turn_id: &str,
        tool_name: &ToolName,
        outcome: ToolCallOutcome,
    ) {
        let mut inner = self.inner();
        if inner.current_turn_id.as_deref() != Some(turn_id) {
            return;
        }
        let Some(turn) = inner.turns.get_mut(turn_id) else {
            return;
        };
        if !turn.account_tokens || turn.active_goal_id.is_none() {
            return;
        }

        match outcome {
            ToolCallOutcome::Completed { success: true } => {
                turn.execution_evidence = TurnExecutionEvidence::Recovered;
                inner.execution_failure_streak = GoalExecutionStreak::Clear;
            }
            ToolCallOutcome::Failed {
                handler_executed: true,
            } if tool_name.is_default_namespace() && tool_name.name == "exec" => {
                if matches!(turn.execution_evidence, TurnExecutionEvidence::NoSignal) {
                    turn.execution_evidence = TurnExecutionEvidence::Unavailable;
                }
            }
            ToolCallOutcome::Completed { success: false }
            | ToolCallOutcome::Failed { .. }
            | ToolCallOutcome::Blocked
            | ToolCallOutcome::Aborted => {}
        }
    }

    pub(crate) fn execution_failure_goal(&self, turn_id: &str) -> Option<String> {
        let mut inner = self.inner();
        if inner.current_turn_id.as_deref() != Some(turn_id) {
            return None;
        }
        let turn = inner.turns.get_mut(turn_id)?;
        if !turn.account_tokens
            || !matches!(turn.execution_evidence, TurnExecutionEvidence::Unavailable)
        {
            return None;
        }
        let goal_id = turn.active_goal_id.clone()?;
        turn.execution_evidence = TurnExecutionEvidence::Counted;
        let qualifying_turns = match &mut inner.execution_failure_streak {
            GoalExecutionStreak::Tracking {
                goal_id: tracked_goal_id,
                qualifying_turns,
            } if *tracked_goal_id == goal_id => {
                *qualifying_turns = qualifying_turns.saturating_add(1);
                *qualifying_turns
            }
            GoalExecutionStreak::Clear | GoalExecutionStreak::Tracking { .. } => {
                inner.execution_failure_streak = GoalExecutionStreak::Tracking {
                    goal_id: goal_id.clone(),
                    qualifying_turns: 1,
                };
                1
            }
        };
        (qualifying_turns >= 3).then_some(goal_id)
    }
}
