use codex_core::context::ContextualUserFragment;
use codex_core::context::InternalContextSource;
use codex_core::context::InternalModelContextFragment;
use codex_core::context::without_update_plan_instructions;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ThreadGoal;
use codex_utils_template::Template;
use serde_json::Value;
use serde_json::json;
use std::sync::LazyLock;

const GOAL_CONTEXT_WORLD_STATE_ID: &str = "goal_context";
const GOAL_CONTEXT_REVISION_START: &str = "<goal_context_revision>";
const GOAL_CONTEXT_REVISION_END: &str = "</goal_context_revision>";

static GOAL_CONTEXT_REVISION_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/goals/continuation.md"),
        "goals/continuation.md",
    )
});

static GOAL_CONTEXT_REVISION_PROMPT_WITHOUT_UPDATE_PLAN: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        &without_update_plan_instructions(include_str!("../templates/goals/continuation.md")),
        "goals/continuation.md",
    )
});

static CONTINUATION_DELTA_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/goals/continuation_delta.md"),
        "goals/continuation_delta.md",
    )
});

static CONTINUATION_DELTA_PROMPT_WITHOUT_UPDATE_PLAN: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        &without_update_plan_instructions(include_str!("../templates/goals/continuation_delta.md")),
        "goals/continuation_delta.md",
    )
});

static BUDGET_LIMIT_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/goals/budget_limit.md"),
        "goals/budget_limit.md",
    )
});

static OBJECTIVE_UPDATED_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/goals/objective_updated.md"),
        "goals/objective_updated.md",
    )
});

fn parse_embedded_template(source: &str, template_name: &str) -> Template {
    match Template::parse(source) {
        Ok(template) => template,
        Err(err) => panic!("embedded template {template_name} is invalid: {err}"),
    }
}

pub(crate) fn budget_limit_steering_item(goal: &ThreadGoal) -> ResponseItem {
    goal_context_input_item("goal", budget_limit_prompt(goal))
}

pub(crate) fn objective_updated_steering_item(goal: &ThreadGoal) -> ResponseItem {
    goal_context_input_item("goal", objective_updated_prompt(goal))
}

pub(crate) fn continuation_delta_steering_item(
    goal: &ThreadGoal,
    update_plan_enabled: bool,
) -> ResponseItem {
    goal_context_input_item(
        "goal_continuation",
        continuation_delta_prompt(goal, update_plan_enabled),
    )
}

pub(crate) fn goal_context_world_state_section(
    goal: &codex_state::ThreadGoal,
    update_plan_enabled: bool,
) -> WorldStateSectionContribution {
    let goal_id = goal.goal_id.clone();
    let objective = goal.objective.clone();
    let rendered_goal_id = escape_xml_text(&goal_id);
    let rendered_objective = escape_xml_text(&objective);
    let retained_goal_revision = format!("<goal_revision>{rendered_goal_id}</goal_revision>");
    let retained_objective = format!("<objective>\n{rendered_objective}\n</objective>");
    let body = goal_context_revision_prompt(goal, update_plan_enabled);
    let retained_body = body.clone();
    let snapshot = json!({
        "goalId": goal_id,
        "objective": objective,
        "updatePlanEnabled": update_plan_enabled,
    });
    let expected_goal_id = goal.goal_id.clone();
    let expected_objective = goal.objective.clone();

    WorldStateSectionContribution::new(GOAL_CONTEXT_WORLD_STATE_ID, snapshot, move |previous| {
        match previous {
            PreviousWorldStateSection::Known(previous)
                if goal_revision_matches(
                    previous,
                    &expected_goal_id,
                    &expected_objective,
                    update_plan_enabled,
                ) =>
            {
                None
            }
            PreviousWorldStateSection::Unknown => None,
            PreviousWorldStateSection::Absent | PreviousWorldStateSection::Known(_) => {
                Some(RenderedWorldStateFragment::new(
                    "user",
                    (GOAL_CONTEXT_REVISION_START, GOAL_CONTEXT_REVISION_END),
                    body.clone(),
                ))
            }
        }
    })
    .with_legacy_matcher(move |role, text| {
        is_legacy_goal_continuation_context(role, text)
            && (update_plan_enabled || !text.contains("If update_plan is available"))
    })
    .with_retained_fragment_matcher(move |role, text| {
        role == "user"
            && text.trim_start().starts_with(GOAL_CONTEXT_REVISION_START)
            && text.contains(&retained_goal_revision)
            && text.contains(&retained_objective)
            && text.contains(&retained_body)
            && text.trim_end().ends_with(GOAL_CONTEXT_REVISION_END)
    })
}

fn goal_context_input_item(source: &'static str, prompt: String) -> ResponseItem {
    ContextualUserFragment::into(InternalModelContextFragment::new(
        InternalContextSource::from_static(source),
        prompt,
    ))
}

fn goal_context_revision_prompt(
    goal: &codex_state::ThreadGoal,
    update_plan_enabled: bool,
) -> String {
    let goal_revision = escape_xml_text(&goal.goal_id);
    let objective = escape_xml_text(&goal.objective);

    let template = if update_plan_enabled {
        &*GOAL_CONTEXT_REVISION_PROMPT_TEMPLATE
    } else {
        &*GOAL_CONTEXT_REVISION_PROMPT_WITHOUT_UPDATE_PLAN
    };
    template
        .render([
            ("goal_revision", goal_revision.as_str()),
            ("objective", objective.as_str()),
        ])
        .unwrap_or_else(|err| {
            panic!("embedded goals/continuation.md template failed to render: {err}")
        })
}

fn continuation_delta_prompt(goal: &ThreadGoal, update_plan_enabled: bool) -> String {
    let tokens_used = goal.tokens_used.to_string();
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());
    let remaining_tokens = goal
        .token_budget
        .map(|budget| (budget - goal.tokens_used).max(0).to_string())
        .unwrap_or_else(|| "unbounded".to_string());

    let template = if update_plan_enabled {
        &*CONTINUATION_DELTA_PROMPT_TEMPLATE
    } else {
        &*CONTINUATION_DELTA_PROMPT_WITHOUT_UPDATE_PLAN
    };
    template
        .render([
            ("tokens_used", tokens_used.as_str()),
            ("token_budget", token_budget.as_str()),
            ("remaining_tokens", remaining_tokens.as_str()),
        ])
        .unwrap_or_else(|err| {
            panic!("embedded goals/continuation_delta.md template failed to render: {err}")
        })
}

fn budget_limit_prompt(goal: &ThreadGoal) -> String {
    let objective = escape_xml_text(&goal.objective);
    let time_used_seconds = goal.time_used_seconds.to_string();
    let tokens_used = goal.tokens_used.to_string();
    let token_budget = goal
        .token_budget
        .map(|budget| budget.to_string())
        .unwrap_or_else(|| "none".to_string());

    BUDGET_LIMIT_PROMPT_TEMPLATE
        .render([
            ("objective", objective.as_str()),
            ("time_used_seconds", time_used_seconds.as_str()),
            ("tokens_used", tokens_used.as_str()),
            ("token_budget", token_budget.as_str()),
        ])
        .unwrap_or_else(|err| {
            panic!("embedded goals/budget_limit.md template failed to render: {err}")
        })
}

fn objective_updated_prompt(goal: &ThreadGoal) -> String {
    let tokens_used = goal.tokens_used.to_string();
    let (token_budget, remaining_tokens) = match goal.token_budget {
        Some(token_budget) => (
            token_budget.to_string(),
            (token_budget - goal.tokens_used).max(0).to_string(),
        ),
        None => ("none".to_string(), "unknown".to_string()),
    };
    OBJECTIVE_UPDATED_PROMPT_TEMPLATE
        .render([
            ("tokens_used", tokens_used.as_str()),
            ("token_budget", token_budget.as_str()),
            ("remaining_tokens", remaining_tokens.as_str()),
        ])
        .unwrap_or_else(|err| {
            panic!("embedded goals/objective_updated.md template failed to render: {err}")
        })
}

fn goal_revision_matches(
    previous: &Value,
    goal_id: &str,
    objective: &str,
    update_plan_enabled: bool,
) -> bool {
    previous.get("goalId").and_then(Value::as_str) == Some(goal_id)
        && previous.get("objective").and_then(Value::as_str) == Some(objective)
        && previous.get("updatePlanEnabled").and_then(Value::as_bool) == Some(update_plan_enabled)
}

fn is_legacy_goal_continuation_context(role: &str, text: &str) -> bool {
    let text = text.trim();
    role == "user"
        && (text.starts_with("<codex_internal_context source=\"goal\">")
            || text.starts_with("<goal_context>"))
        && text.contains("Continue working toward the active thread goal.")
        && text.contains("<objective>")
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
