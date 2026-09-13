use super::*;
use crate::context::world_state::WorldState;
use codex_utils_string::approx_bytes_for_tokens;
use pretty_assertions::assert_eq;

fn state(
    model: &str,
    personality: Option<Personality>,
    previous: Option<(&str, Option<Personality>)>,
    personality_is_baked: bool,
) -> PersonalityState {
    PersonalityState::new(
        model,
        personality,
        previous.map(|(model, _)| model),
        previous.and_then(|(_, personality)| personality),
        personality.map(|personality| format!("instructions for {personality:?}")),
        personality_is_baked,
    )
}

#[test]
fn initial_personality_renders_only_when_missing_from_base_instructions() {
    let separate = state(
        "gpt-test",
        Some(Personality::Friendly),
        /*previous*/ None,
        /*personality_is_baked*/ false,
    );
    let baked = state(
        "gpt-test",
        Some(Personality::Friendly),
        /*previous*/ None,
        /*personality_is_baked*/ true,
    );

    assert_eq!(
        separate
            .render_diff(PreviousSectionState::Absent)
            .expect("separate personality should render")
            .markers(),
        PersonalitySpecInstructions::type_markers()
    );
    assert!(baked.render_diff(PreviousSectionState::Absent).is_none());
}

#[test]
fn personality_changes_render_without_repeating_model_changes() {
    let previous = PersonalitySnapshot {
        model: "gpt-test".to_string(),
        personality: Some(Personality::Friendly),
    };
    let changed = state(
        "gpt-test",
        Some(Personality::Pragmatic),
        Some(("gpt-test", Some(Personality::Friendly))),
        /*personality_is_baked*/ true,
    );
    let model_changed = state(
        "gpt-next",
        Some(Personality::Pragmatic),
        Some(("gpt-test", Some(Personality::Friendly))),
        /*personality_is_baked*/ true,
    );

    assert_eq!(
        changed
            .render_diff(PreviousSectionState::Known(&previous))
            .expect("changed personality should render")
            .markers(),
        PersonalitySpecInstructions::type_markers()
    );
    assert!(
        model_changed
            .render_diff(PreviousSectionState::Known(&previous))
            .is_none()
    );
    assert!(
        model_changed
            .render_diff(PreviousSectionState::Unknown)
            .is_none()
    );
    assert!(
        model_changed
            .render_diff(PreviousSectionState::Absent)
            .is_none()
    );
}

#[test]
fn persisted_personality_does_not_require_a_retained_update() {
    let state = state(
        "gpt-test",
        Some(Personality::Friendly),
        Some(("gpt-test", Some(Personality::Friendly))),
        /*personality_is_baked*/ false,
    );
    let mut world_state = WorldState::default();
    world_state.add_section(state);
    let snapshot = world_state.snapshot();

    assert!(
        world_state
            .render_history_diff(Some(&snapshot), &[])
            .is_empty()
    );
}

#[test]
fn personality_change_bounds_the_complete_developer_message() {
    let previous = PersonalitySnapshot {
        model: "gpt-test".to_string(),
        personality: Some(Personality::Friendly),
    };
    let state = PersonalityState::new(
        "gpt-test",
        Some(Personality::Pragmatic),
        Some("gpt-test"),
        Some(Personality::Friendly),
        Some("oversized personality ".repeat(20_000)),
        /*personality_is_baked*/ true,
    );

    let rendered = state
        .render_diff(PreviousSectionState::Known(&previous))
        .expect("personality change should render")
        .render();

    assert!(
        rendered.len()
            <= approx_bytes_for_tokens(
                crate::context::personality_spec_instructions::MAX_PERSONALITY_SPEC_INSTRUCTIONS_TOKENS,
            )
    );
    assert!(rendered.contains("truncated"));
    assert!(rendered.starts_with("<personality_spec>"));
    assert!(rendered.ends_with("</personality_spec>"));
}
