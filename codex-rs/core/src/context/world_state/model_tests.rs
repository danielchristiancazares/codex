use super::*;
use pretty_assertions::assert_eq;

#[test]
fn model_change_renders_when_persisted_or_inferred_from_previous_turn() {
    let state = ModelInstructionsState::new("gpt-new", Some("gpt-old"), "instructions".into());
    let previous = ModelInstructionsSnapshot::Legacy("gpt-old".to_string());

    for previous in [
        PreviousSectionState::Known(&previous),
        PreviousSectionState::Unknown,
        PreviousSectionState::Absent,
    ] {
        assert_eq!(
            state
                .render_diff(previous)
                .expect("model change should render")
                .markers(),
            ModelSwitchInstructions::type_markers()
        );
    }
}

#[test]
fn unchanged_model_does_not_render() {
    let state = ModelInstructionsState::new("gpt-test", Some("gpt-test"), "instructions".into());
    let previous = ModelInstructionsSnapshot::Legacy("gpt-test".to_string());

    assert!(
        state
            .render_diff(PreviousSectionState::Known(&previous))
            .is_none()
    );
    assert!(state.render_diff(PreviousSectionState::Absent).is_none());
}

#[test]
fn model_change_with_identical_instructions_does_not_render() {
    let previous = ModelInstructionsState::new("gpt-old", None, "shared instructions".into());
    let current =
        ModelInstructionsState::new("gpt-new", Some("gpt-old"), "shared instructions".into());

    assert!(
        current
            .render_diff(PreviousSectionState::Known(&previous.snapshot()))
            .is_none()
    );
}

#[test]
fn model_change_with_different_instructions_renders() {
    let previous = ModelInstructionsState::new("gpt-old", None, "old instructions".into());
    let current =
        ModelInstructionsState::new("gpt-new", Some("gpt-old"), "new instructions".into());

    assert_eq!(
        current
            .render_diff(PreviousSectionState::Known(&previous.snapshot()))
            .map(|fragment| fragment.render()),
        Some(ModelSwitchInstructions::new("new instructions").render())
    );
}

#[test]
fn instruction_change_on_same_model_does_not_render_model_switch() {
    let previous = ModelInstructionsState::new("gpt-test", None, "old instructions".into());
    let current =
        ModelInstructionsState::new("gpt-test", Some("gpt-test"), "new instructions".into());

    assert!(
        current
            .render_diff(PreviousSectionState::Known(&previous.snapshot()))
            .is_none()
    );
}
