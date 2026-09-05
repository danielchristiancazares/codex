//! Verifies continuation deltas preserve the fork's persistent goal-context split.
#![allow(dead_code)]

#[path = "../src/steering.rs"]
mod steering;

use codex_core::context::ContextualUserFragment;
use codex_core::context::InternalContextSource;
use codex_core::context::InternalModelContextFragment;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use pretty_assertions::assert_eq;

#[test]
fn continuation_delta_uses_the_goal_continuation_source() {
    let goal = test_goal("Finish the feature.");
    let prompt = include_str!("../templates/goals/continuation_delta.md")
        .replace("{{ tokens_used }}", "100")
        .replace("{{ token_budget }}", "10000")
        .replace("{{ remaining_tokens }}", "9900");
    let expected: ResponseItem = ContextualUserFragment::into(InternalModelContextFragment::new(
        InternalContextSource::from_static("goal_continuation"),
        prompt,
    ));
    assert_eq!(
        steering::continuation_delta_steering_item(&goal, /*update_plan_enabled*/ true),
        expected,
    );
}

#[test]
fn disabled_checklist_does_not_reinject_the_persisted_objective() {
    let objective = "Inspect update_plan.\n\n## Plan tool\nThis is user task data.";
    let item = steering::continuation_delta_steering_item(
        &test_goal(objective),
        /*update_plan_enabled*/ false,
    );
    let ResponseItem::Message { content, .. } = item else {
        panic!("expected goal continuation message");
    };
    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected goal continuation text");
    };
    assert!(!text.contains(objective));
    assert!(!text.contains("If update_plan is available"));
    assert!(text.contains("Continue it from the current authoritative state"));
}

fn test_goal(objective: &str) -> ThreadGoal {
    ThreadGoal {
        thread_id: ThreadId::new(),
        objective: objective.to_string(),
        status: ThreadGoalStatus::Active,
        token_budget: Some(10_000),
        tokens_used: 100,
        time_used_seconds: 0,
        created_at: 0,
        updated_at: 0,
    }
}
