use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

fn search_output(call_id: &str, tools: Vec<Value>) -> ResponseItem {
    ResponseItem::ToolSearchOutput {
        id: None,
        call_id: Some(call_id.to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn output_tools(item: &ResponseItem) -> &[Value] {
    let ResponseItem::ToolSearchOutput { tools, .. } = item else {
        panic!("expected tool search output");
    };
    tools
}

#[test]
fn exact_function_definitions_are_retained_once() {
    let definition = json!({
        "type": "function",
        "name": "calendar_create",
        "description": "Create an event",
        "parameters": {"type": "object"},
    });
    let mut state = ToolDiscoveryState::default();
    let mut first = search_output("search-1", vec![definition.clone()]);
    let mut second = search_output("search-2", vec![definition.clone()]);

    state.deduplicate_response_item(&mut first);
    state.deduplicate_response_item(&mut second);

    assert_eq!(output_tools(&first), &[definition]);
    assert_eq!(output_tools(&second), Vec::<Value>::new());
}

#[test]
fn overlapping_namespaces_retain_only_new_leaf_definitions() {
    let first_tool = json!({"type": "function", "name": "first"});
    let shared_tool = json!({"type": "function", "name": "shared"});
    let latest_tool = json!({"type": "function", "name": "latest"});
    let namespace = |tools| {
        json!({
            "type": "namespace",
            "name": "calendar",
            "description": "Calendar tools",
            "tools": tools,
        })
    };
    let mut state = ToolDiscoveryState::default();
    let mut first = search_output(
        "search-1",
        vec![namespace(vec![first_tool.clone(), shared_tool.clone()])],
    );
    let mut second = search_output(
        "search-2",
        vec![namespace(vec![shared_tool, latest_tool.clone()])],
    );

    state.deduplicate_response_item(&mut first);
    state.deduplicate_response_item(&mut second);

    assert_eq!(
        output_tools(&first),
        &[namespace(vec![
            first_tool,
            json!({"type": "function", "name": "shared"}),
        ])]
    );
    assert_eq!(output_tools(&second), &[namespace(vec![latest_tool])]);
}

#[test]
fn changed_schema_revision_is_retained() {
    let mut state = ToolDiscoveryState::default();
    let mut first = search_output(
        "search-1",
        vec![json!({"type": "function", "name": "calendar", "description": "v1"})],
    );
    let revised = json!({"type": "function", "name": "calendar", "description": "v2"});
    let mut second = search_output("search-2", vec![revised.clone()]);

    state.deduplicate_response_item(&mut first);
    state.deduplicate_response_item(&mut second);

    assert_eq!(output_tools(&second), &[revised]);
}
