use http::HeaderName;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use super::*;

const TEST_MACHINE_ID: &str = "4f8c2f5df054b1e465c8f9d9af3b391a4718b02ad7c3d0f8e83d4f6978de1451";

fn entry(value: Value) -> CopilotModelEntry {
    serde_json::from_value(value).expect("valid Copilot model")
}

#[test]
fn model_filter_requires_picker_and_websocket_responses() {
    let websocket = entry(json!({
        "id": "gpt-5.6-sol",
        "model_picker_enabled": true,
        "supported_endpoints": ["/responses", "ws:/responses"],
        "capabilities": {"type": "chat"}
    }));
    let http_only = entry(json!({
        "id": "grok-code-fast-1",
        "model_picker_enabled": true,
        "supported_endpoints": ["/responses"],
        "capabilities": {"type": "chat"}
    }));
    let websocket_only = entry(json!({
        "id": "gpt-websocket-only",
        "model_picker_enabled": true,
        "supported_endpoints": ["ws:/responses"],
        "capabilities": {"type": "chat"}
    }));
    let hidden = entry(json!({
        "id": "gpt-hidden",
        "model_picker_enabled": false,
        "supported_endpoints": ["/responses", "ws:/responses"],
        "capabilities": {"type": "chat"}
    }));

    assert!(is_websocket_responses_model(&websocket));
    assert!(is_websocket_responses_model(&websocket_only));
    assert!(!is_websocket_responses_model(&http_only));
    assert!(!is_websocket_responses_model(&hidden));
}

#[test]
fn catalog_keeps_every_eligible_websocket_responses_model() {
    let entries = vec![
        entry(json!({
            "id": "gpt-5.6-sol",
            "model_picker_enabled": true,
            "supported_endpoints": ["ws:/responses"],
            "capabilities": {"type": "chat"}
        })),
        entry(json!({
            "id": "claude-sonnet-4.5",
            "model_picker_enabled": true,
            "supported_endpoints": ["ws:/responses"],
            "capabilities": {"type": "chat"}
        })),
        entry(json!({
            "id": "http-only",
            "model_picker_enabled": true,
            "supported_endpoints": ["/responses"],
            "capabilities": {"type": "chat"}
        })),
    ];

    assert_eq!(
        available_models(entries, &[])
            .into_iter()
            .map(|model| model.slug)
            .collect::<Vec<_>>(),
        vec!["gpt-5.6-sol", "claude-sonnet-4.5"]
    );
}

#[test]
fn translation_uses_live_limits_reasoning_and_websocket_priority() {
    let info = translate_entry(
        entry(json!({
            "id": "gpt-5.6-sol",
            "name": "GPT-5.6 Sol",
            "vendor": "OpenAI",
            "model_picker_enabled": true,
            "supported_endpoints": ["/responses", "ws:/responses"],
            "capabilities": {
                "type": "chat",
                "limits": {
                    "max_context_window_tokens": 1050000,
                    "max_output_tokens": 128000
                },
                "supports": {
                    "reasoning_effort": ["low", "medium", "high"],
                    "vision": false
                }
            }
        })),
        &[],
    );

    assert_eq!(info.display_name, "GPT-5.6 Sol");
    assert_eq!(info.priority, 0);
    assert_eq!(info.context_window, Some(400_000));
    assert_eq!(info.max_context_window, Some(922_000));
    assert_eq!(info.default_reasoning_level, Some(ReasoningEffort::Medium));
    assert_eq!(
        info.supported_reasoning_levels
            .iter()
            .map(|preset| preset.effort.clone())
            .collect::<Vec<_>>(),
        vec![
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High
        ]
    );
    assert_eq!(info.input_modalities, vec![InputModality::Text]);
    assert!(!info.supports_search_tool);
    assert!(!info.supports_reasoning_summary_parameter);
    assert!(!info.supports_image_detail_original);
    assert_eq!(info.tool_mode, Some(ToolMode::Direct));
}

#[test]
fn model_headers_use_copilot_identity() {
    let source = HeaderMap::from_iter([
        (
            http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret"),
        ),
        (
            HeaderName::from_static("x-github-api-version"),
            HeaderValue::from_static("2026-08-01"),
        ),
    ]);

    let headers = models_headers(&source, Some(TEST_MACHINE_ID));

    assert!(!headers.contains_key("x-github-api-version"));
    assert!(!headers.contains_key("editor-version"));
    assert!(!headers.contains_key("editor-plugin-version"));
    assert!(headers.contains_key("x-interaction-id"));
    let interaction_id = headers
        .get("x-interaction-id")
        .and_then(|value| value.to_str().ok())
        .expect("interaction ID");
    Uuid::parse_str(interaction_id).expect("UUID interaction ID");
    assert!(!headers.contains_key("x-request-id"));
    assert_eq!(
        headers
            .get("x-initiator")
            .and_then(|value| value.to_str().ok()),
        Some("user")
    );
    assert_eq!(
        headers
            .get("copilot-integration-id")
            .and_then(|value| value.to_str().ok()),
        Some("copilot-developer-cli")
    );
    assert_eq!(
        headers
            .get("copilot-harness-id")
            .and_then(|value| value.to_str().ok()),
        Some("copilot-sdk")
    );
    assert_eq!(
        headers
            .get("openai-intent")
            .and_then(|value| value.to_str().ok()),
        Some("conversation-agent")
    );
    assert_eq!(
        headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        Some("GitHubCopilotCLI/1.0.80")
    );
    assert_eq!(
        headers
            .get("x-client-application")
            .and_then(|value| value.to_str().ok()),
        Some("copilot-cli")
    );
    assert_eq!(
        headers
            .get("x-client-machine-id")
            .and_then(|value| value.to_str().ok()),
        Some(TEST_MACHINE_ID)
    );
    assert_eq!(
        headers
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer secret")
    );
}

#[test]
fn top_level_model_data_is_required() {
    let error = serde_json::from_value::<CopilotModelsResponse>(json!({}))
        .expect_err("missing model data must fail");

    assert!(error.to_string().contains("missing field `data`"));
}
