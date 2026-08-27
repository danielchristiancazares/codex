use codex_tools::ToolSpec;
use pretty_assertions::assert_eq;

use super::parse_handoff;
use crate::context::MAX_CONTEXT_WINDOW_HANDOFF_BYTES;
use crate::function_tool::FunctionCallError;
use crate::tools::handlers::new_context_window_spec::create_new_context_window_tool;

#[test]
fn new_context_spec_requires_handoff() {
    let ToolSpec::Function(spec) = create_new_context_window_tool() else {
        panic!("new_context must be a function tool");
    };

    assert_eq!(spec.parameters.required, Some(vec!["handoff".to_string()]));
    assert_eq!(spec.parameters.additional_properties, Some(false.into()));
    assert!(
        spec.parameters
            .properties
            .as_ref()
            .is_some_and(|properties| properties.contains_key("handoff"))
    );
}

#[test]
fn parse_handoff_trims_markdown() {
    let handoff = parse_handoff(r#"{"handoff":"  # Active task\n\nContinue.  "}"#).unwrap();

    assert_eq!(handoff, "# Active task\n\nContinue.");
}

#[test]
fn parse_handoff_rejects_empty_content() {
    let result = parse_handoff(r#"{"handoff":"  \n  "}"#);
    let Err(FunctionCallError::RespondToModel(message)) = result else {
        panic!("empty handoff should be rejected");
    };

    assert_eq!(
        message,
        "`handoff` must contain a self-contained plain-Markdown handoff."
    );
}

#[test]
fn parse_handoff_rejects_oversized_content() {
    let handoff = "word ".repeat(MAX_CONTEXT_WINDOW_HANDOFF_BYTES);
    let arguments = serde_json::json!({ "handoff": handoff }).to_string();
    let result = parse_handoff(&arguments);
    let Err(FunctionCallError::RespondToModel(message)) = result else {
        panic!("oversized handoff should be rejected");
    };

    assert!(message.contains("shorten it to at most 8000 UTF-8 bytes"));
}
