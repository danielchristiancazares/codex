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
fn rebuilding_visibility_preserves_surviving_history_and_allows_rediscovery_after_compaction() {
    let definition = json!({"type": "function", "name": "calendar"});
    let items = vec![
        ResponseItemEnvelope::new(search_output("search-1", vec![definition.clone()])),
        ResponseItemEnvelope::new(search_output("search-2", vec![definition.clone()])),
    ];
    let original = items.clone();
    let mut state = ToolDiscoveryState::default();
    state.rebuild(&items);
    assert_eq!(items, original);
    let mut repeated = search_output("search-3", vec![definition.clone()]);
    state.observe(&mut repeated);
    assert_eq!(output_tools(&repeated), Vec::<Value>::new());
    state.rebuild(&[]);
    let mut rediscovered = search_output("search-4", vec![definition.clone()]);
    state.observe(&mut rediscovered);
    assert_eq!(output_tools(&rediscovered), &[definition]);
}

#[test]
fn oldest_surviving_schema_is_reused_after_more_than_a_thousand_discoveries() {
    let items = (0..1_100)
        .map(|index| {
            ResponseItemEnvelope::new(search_output(
                &format!("search-{index}"),
                vec![json!({"type": "function", "name": format!("tool_{index}")})],
            ))
        })
        .collect::<Vec<_>>();
    let mut state = ToolDiscoveryState::default();
    state.rebuild(&items);
    let oldest = json!({"type": "function", "name": "tool_0"});
    let mut repeated = search_output("repeat-oldest", vec![oldest.clone()]);
    state.observe(&mut repeated);
    assert_eq!(output_tools(&repeated), Vec::<Value>::new());

    state.rebuild(&items[1..]);
    let mut rediscovered = search_output("rediscover-oldest", vec![oldest.clone()]);
    state.observe(&mut rediscovered);
    assert_eq!(output_tools(&rediscovered), &[oldest]);
}

#[test]
fn changed_namespace_metadata_is_published_with_the_definition() {
    let mut state = ToolDiscoveryState::default();
    let namespace = |description| {
        json!({
            "type": "namespace",
            "name": "calendar",
            "description": description,
            "tools": [{"type": "function", "name": "read"}],
        })
    };
    state.observe(&mut search_output("search-1", vec![namespace("original")]));
    let updated = namespace("updated instructions");
    let mut second = search_output("search-2", vec![updated.clone()]);
    state.observe(&mut second);
    assert_eq!(output_tools(&second), &[updated]);
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
