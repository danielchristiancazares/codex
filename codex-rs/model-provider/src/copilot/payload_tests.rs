use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use super::Initiator;
use super::initiator;
use super::normalize_websocket;

#[test]
fn websocket_normalization_preserves_envelope_and_applies_all_transformations() {
    let mut payload = json!({
        "type": "response.create",
        "model": "gpt-5.5",
        "previous_response_id": "resp-old",
        "service_tier": "default",
        "stream": true,
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
            "service_tier": null,
            "stream": true,
            "client_metadata": {"traceparent": "trace-value"},
            "input": [
                {"type": "compaction", "encrypted_content": "opaque"},
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "retained"}]
                }
            ],
            "tools": [
                {
                    "type": "function",
                    "name": "apply_patch",
                    "description": "Use the apply_patch tool to edit files",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "input": {
                                "type": "string",
                                "description": "The entire contents of the apply_patch command"
                            }
                        },
                        "required": ["input"]
                    },
                    "strict": false
                },
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
fn initiator_and_interaction_type_follow_the_last_input_role() {
    let payloads = [
        json!({"input": [{"type": "message", "role": "user"}]}),
        json!({"input": [{"type": "message", "role": "assistant"}]}),
        json!({"input": [{"type": "message", "role": "ASSISTANT"}]}),
        json!({"input": [{"type": "function_call_output"}]}),
        json!({"input": []}),
        json!({}),
    ];

    let actual = payloads
        .iter()
        .map(|payload| {
            let initiator = initiator(payload);
            (initiator, initiator.as_str(), initiator.interaction_type())
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (Initiator::User, "user", "conversation-user"),
            (Initiator::Agent, "agent", "conversation-agent"),
            (Initiator::Agent, "agent", "conversation-agent"),
            (Initiator::Agent, "agent", "conversation-agent"),
            (Initiator::User, "user", "conversation-user"),
            (Initiator::User, "user", "conversation-user"),
        ]
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
            "service_tier": null,
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
