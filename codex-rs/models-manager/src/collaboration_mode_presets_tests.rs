use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preset_names_use_mode_display_names() {
    assert_eq!(plan_preset().name, ModeKind::Plan.display_name());
    assert_eq!(default_preset().name, ModeKind::Default.display_name());
    assert_eq!(plan_preset().model, None);
    assert_eq!(
        plan_preset().reasoning_effort,
        NullableField::Value(ReasoningEffort::Medium)
    );
    assert_eq!(default_preset().model, None);
    assert_eq!(default_preset().reasoning_effort, NullableField::Omitted);
}

#[test]
fn default_mode_instructions_replace_mode_names_placeholder() {
    let NullableField::Value(default_instructions) = default_preset().developer_instructions else {
        panic!("default preset should include instructions");
    };

    assert!(!default_instructions.contains("{{KNOWN_MODE_NAMES}}"));

    let known_mode_names = format_mode_names(&TUI_VISIBLE_COLLABORATION_MODES);
    let expected_snippet = format!("Known mode names are {known_mode_names}.");
    assert!(default_instructions.contains(&expected_snippet));

    assert!(default_instructions.contains(
        "Use the `request_user_input` tool only when it is listed in the available tools"
    ));
    assert!(
        default_instructions.contains("ask the user directly with a concise plain-text question")
    );
}
