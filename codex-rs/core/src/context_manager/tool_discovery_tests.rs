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

fn search_call(call_id: &str) -> ResponseItem {
    ResponseItem::ToolSearchCall {
        id: None,
        call_id: Some(call_id.to_string()),
        status: None,
        execution: "client".to_string(),
        arguments: json!({"query": "calendar"}),
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

#[test]
fn pending_exchange_restores_the_full_latest_result_until_model_continuation() {
    let definition = json!({"type": "function", "name": "calendar"});
    let mut state = ToolDiscoveryState::default();
    let mut first = search_output("search-1", vec![definition.clone()]);
    state.deduplicate_response_item(&mut first);
    state.note_model_generated_item();

    let call = search_call("search-2");
    let full_output = search_output("search-2", vec![definition]);
    let mut deduplicated_output = full_output.clone();
    state.deduplicate_response_item(&mut deduplicated_output);
    let items = vec![
        ResponseItemEnvelope::new(call.clone()),
        ResponseItemEnvelope::new(deduplicated_output),
    ];

    assert_eq!(
        state.pending_exchange(&items),
        vec![
            ResponseItemEnvelope::new(call),
            ResponseItemEnvelope::new(full_output),
        ]
    );

    let pending_state = state.clone();
    state.note_model_generated_item();
    state.restore_pending_output_from(&pending_state);
    assert_eq!(state.pending_exchange(&items), Vec::new());
}

#[test]
fn compaction_projection_clears_schema_bodies_and_preserves_output_envelopes() {
    let mut output = search_output(
        "search-1",
        vec![json!({"type": "function", "name": "calendar"})],
    );
    let expected = search_output("search-1", Vec::new());

    assert_eq!(strip_tool_search_schemas(std::iter::once(&mut output)), 1);
    assert_eq!(output, expected);
    assert_eq!(strip_tool_search_schemas(std::iter::once(&mut output)), 0);
}
