use super::*;
use codex_connectors::AppInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn discoverable_tool_enums_use_expected_wire_names() {
    assert_eq!(
        json!({
            "tool_type": DiscoverableToolType::Connector,
            "action_type": DiscoverableToolAction::Install,
        }),
        json!({
            "tool_type": "connector",
            "action_type": "install",
        })
    );
}

#[test]
fn filter_request_plugin_install_discoverable_tools_for_codex_tui_omits_plugins() {
    let discoverable_tools = vec![
        DiscoverableTool::Connector(Box::new(AppInfo {
            id: "connector_google_calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events and schedules.".to_string()),
            logo_url: None,
            logo_url_dark: None,
            icon_assets: None,
            icon_dark_assets: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/google-calendar".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        })),
        DiscoverableTool::Plugin(Box::new(DiscoverablePluginInfo {
            id: "slack@openai-curated".to_string(),
            remote_plugin_id: None,
            name: "Slack".to_string(),
            description: Some("Search Slack messages".to_string()),
            has_skills: true,
            mcp_server_names: vec!["slack".to_string()],
            app_connector_ids: vec!["connector_slack".to_string()],
        })),
    ];

    assert_eq!(
        filter_request_plugin_install_discoverable_tools_for_client(
            discoverable_tools,
            Some("codex-tui"),
        ),
        vec![DiscoverableTool::Connector(Box::new(AppInfo {
            id: "connector_google_calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events and schedules.".to_string()),
            logo_url: None,
            logo_url_dark: None,
            icon_assets: None,
            icon_dark_assets: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/google-calendar".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }))]
    );
}

#[test]
fn tool_search_output_is_bounded_by_leaf_count() {
    let tools = (0..40)
        .map(|index| {
            json!({
                "type": "function",
                "name": format!("tool_{index}"),
                "description": "small"
            })
        })
        .collect();

    let bounded = bound_tool_search_output(tools);

    assert_eq!(bounded.len(), TOOL_SEARCH_MAX_RESULTS);
    assert!(serde_json::to_vec(&bounded).unwrap().len() <= TOOL_SEARCH_MAX_OUTPUT_BYTES);
}

#[test]
fn tool_search_output_bounds_nested_namespace_leaves() {
    let tools = vec![json!({
        "type": "namespace",
        "name": "calendar",
        "description": "calendar tools",
        "tools": (0..40)
            .map(|index| json!({"type": "function", "name": format!("tool_{index}")}))
            .collect::<Vec<_>>()
    })];

    let bounded = bound_tool_search_output(tools);

    assert_eq!(bounded.len(), 1);
    assert_eq!(
        bounded[0]["tools"].as_array().map(Vec::len),
        Some(TOOL_SEARCH_MAX_RESULTS)
    );
    assert!(serde_json::to_vec(&bounded).unwrap().len() <= TOOL_SEARCH_MAX_OUTPUT_BYTES);
}

#[test]
fn tool_search_output_drops_an_oversized_definition() {
    let tools = vec![json!({
        "type": "function",
        "name": "oversized",
        "description": "x".repeat(TOOL_SEARCH_MAX_OUTPUT_BYTES)
    })];

    assert_eq!(
        bound_tool_search_output(tools),
        Vec::<serde_json::Value>::new()
    );
}
