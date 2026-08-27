use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use super::normalize_websocket;

#[test]
fn websocket_normalization_preserves_envelope_and_applies_all_transformations() {
    let mut payload = json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "previous_response_id": "resp-old",
        "service_tier": "default",
        "stream": true,
        "stream_options": {"include_usage": true},
        "tool_choice": "auto",
        "prompt_cache_key": "codex-session",
        "client_metadata": {"traceparent": "trace-value"},
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "discarded"}]
            },
            {"type": "compaction", "encrypted_content": "opaque"},
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "retained"}]
            }
        ],
        "tools": [
            {"type": "web_search"},
            {"type": "web_search_preview"},
            {"type": "custom", "name": "apply_patch", "format": {"type": "grammar"}},
            {
                "type": "function",
                "name": "shell",
                "parameters": {"type": "object", "properties": {}}
            }
        ]
    });

    normalize_websocket(&mut payload);

    assert_eq!(
        payload,
        json!({
            "type": "response.create",
            "model": "gpt-5.5",
            "previous_response_id": "resp-old",
            "input": [
                {"type": "compaction", "encrypted_content": "opaque"},
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "retained"}]
                }
            ],
            "tools": [
                {"type": "custom", "name": "apply_patch", "format": {"type": "grammar"}},
                {
                    "type": "function",
                    "name": "shell",
                    "parameters": {"type": "object", "properties": {}}
                }
            ]
        })
    );
}

#[test]
fn normalization_retains_input_from_the_latest_compaction() {
    let mut payload = json!({
        "type": "response.create",
        "input": [
            {"type": "message", "role": "user", "content": "old"},
            {"type": "compaction", "encrypted_content": "first"},
            {"type": "message", "role": "assistant", "content": "middle"},
            {"type": "compaction", "encrypted_content": "latest"},
            {"type": "message", "role": "user", "content": "new"}
        ]
    });

    normalize_websocket(&mut payload);

    assert_eq!(
        payload,
        json!({
            "type": "response.create",
            "input": [
                {"type": "compaction", "encrypted_content": "latest"},
                {"type": "message", "role": "user", "content": "new"}
            ]
        })
    );
}

#[test]
fn normalization_leaves_non_object_payload_shape_intact() {
    let mut payload = Value::Array(vec![json!({"type": "response.create"})]);

    normalize_websocket(&mut payload);

    assert_eq!(
        payload,
        Value::Array(vec![json!({"type": "response.create"})])
    );
}
