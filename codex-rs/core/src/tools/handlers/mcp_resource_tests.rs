use super::*;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputContentItem;
use pretty_assertions::assert_eq;
use rmcp::model::ResourceContents;
use serde_json::json;

fn resource(uri: &str, name: &str) -> Resource {
    Resource::new(uri, name)
}

fn template(uri_template: &str, name: &str) -> ResourceTemplate {
    ResourceTemplate::new(uri_template, name)
}

#[test]
fn resource_with_server_serializes_server_field() {
    let entry =
        ResourceListingEntry::with_server("test".to_string(), resource("memo://id", "memo"));
    let value = serde_json::to_value(&entry).expect("serialize resource");

    assert_eq!(value["server"], json!("test"));
    assert_eq!(value["uri"], json!("memo://id"));
    assert_eq!(value["name"], json!("memo"));
}

#[test]
fn list_resources_payload_from_single_server_emits_server_once_and_copies_cursor() {
    let resources = (0..50)
        .map(|index| resource(&format!("memo://{index}"), &format!("memo-{index}")))
        .collect();
    let mut result = ListResourcesResult::with_all_items(resources);
    result.next_cursor = Some("cursor-1".to_string());
    let payload = ListResourcesPayload::from_single_server("srv".to_string(), result);
    let value = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        value,
        json!({
            "server": "srv",
            "resources": (0..50)
                .map(|index| json!({
                    "uri": format!("memo://{index}"),
                    "name": format!("memo-{index}"),
                }))
                .collect::<Vec<_>>(),
            "nextCursor": "cursor-1",
        })
    );
}

#[test]
fn list_resources_payload_from_all_servers_is_sorted() {
    let mut map = HashMap::new();
    map.insert("beta".to_string(), vec![resource("memo://b-1", "b-1")]);
    map.insert(
        "alpha".to_string(),
        vec![resource("memo://a-1", "a-1"), resource("memo://a-2", "a-2")],
    );

    let payload = ListResourcesPayload::from_all_servers(map);
    let value = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(
        value,
        json!({
            "resources": [
                {"server": "alpha", "uri": "memo://a-1", "name": "a-1"},
                {"server": "alpha", "uri": "memo://a-2", "name": "a-2"},
                {"server": "beta", "uri": "memo://b-1", "name": "b-1"},
            ],
        })
    );
}

#[test]
fn call_tool_result_from_content_marks_success() {
    let result = call_tool_result_from_content("{}", Some(true));
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content.len(), 1);
}

#[test]
fn parse_arguments_handles_empty_and_json() {
    assert!(
        parse_arguments(" \n\t").unwrap().is_none(),
        "expected None for empty arguments"
    );

    assert!(
        parse_arguments("null").unwrap().is_none(),
        "expected None for null arguments"
    );

    let value = parse_arguments(r#"{"server":"figma"}"#)
        .expect("parse json")
        .expect("value present");
    assert_eq!(value["server"], json!("figma"));
}

#[test]
fn list_resource_args_normalizes_server_and_cursor() {
    let args: ListResourceArgs = serde_json::from_value(json!({
        "server": "  hosted  ",
        "cursor": "  next-page  "
    }))
    .expect("parse resource-list arguments");

    assert_eq!(
        args.normalized(),
        ListResourceArgs {
            server: Some("hosted".to_string()),
            cursor: Some("next-page".to_string()),
        }
    );
}

#[test]
fn template_with_server_serializes_server_field() {
    let entry =
        ResourceListingEntry::with_server("srv".to_string(), template("memo://{id}", "memo"));
    let value = serde_json::to_value(&entry).expect("serialize template");

    assert_eq!(
        value,
        json!({
            "server": "srv",
            "uriTemplate": "memo://{id}",
            "name": "memo"
        })
    );
}

#[test]
fn list_resource_templates_payload_from_single_server_omits_child_server() {
    let payload = ListResourceTemplatesPayload::from_single_server(
        "srv".to_string(),
        ListResourceTemplatesResult::with_all_items(vec![template("memo://{id}", "memo")]),
    );

    assert_eq!(
        serde_json::to_value(payload).expect("serialize resource templates"),
        json!({
            "server": "srv",
            "resourceTemplates": [{"uriTemplate": "memo://{id}", "name": "memo"}],
        })
    );
}

#[test]
fn list_resource_templates_payload_from_all_servers_is_sorted() {
    let mut templates_by_server = HashMap::new();
    templates_by_server.insert(
        "beta".to_string(),
        vec![template("memo://beta/{id}", "beta")],
    );
    templates_by_server.insert(
        "alpha".to_string(),
        vec![template("memo://alpha/{id}", "alpha")],
    );

    let payload = ListResourceTemplatesPayload::from_all_servers(templates_by_server);

    assert_eq!(
        serde_json::to_value(payload).expect("serialize resource templates"),
        json!({
            "resourceTemplates": [
                {"server": "alpha", "uriTemplate": "memo://alpha/{id}", "name": "alpha"},
                {"server": "beta", "uriTemplate": "memo://beta/{id}", "name": "beta"}
            ]
        })
    );
}

#[test]
fn serialize_function_output_preserves_small_payload() {
    let payload = json!({"server": "hosted", "resources": []});
    let expected = serde_json::to_string(&payload).expect("serialize payload");

    let output = serialize_function_output(payload, TruncationPolicy::Bytes(1_024))
        .expect("serialize function output")
        .into_text();

    assert_eq!(output, expected);
}

#[test]
fn read_resource_projection_preserves_text_with_compact_identity_header() {
    let payload = ReadResourcePayload {
        server: "hosted".to_string(),
        uri: "skill://example/SKILL.md".to_string(),
        result: ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
            uri: "skill://example/SKILL.md".to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: "# Example".to_string(),
            meta: None,
        }]),
    };

    let output =
        read_mcp_resource::project_read_resource_output(payload, TruncationPolicy::Bytes(8_000))
            .expect("project text resource");

    assert_eq!(
        output.body,
        vec![FunctionCallOutputContentItem::InputText {
            text:
                "MCP resource `skill://example/SKILL.md` from `hosted` (text/markdown)\n# Example"
                    .to_string(),
        }]
    );
}

#[test]
fn read_resource_projection_emits_image_as_typed_content() {
    let payload = ReadResourcePayload {
        server: "hosted".to_string(),
        uri: "image://example".to_string(),
        result: ReadResourceResult::new(vec![ResourceContents::BlobResourceContents {
            uri: "image://example".to_string(),
            mime_type: Some("image/png".to_string()),
            blob: "aW1hZ2UtYnl0ZXM=".to_string(),
            meta: None,
        }]),
    };

    let output =
        read_mcp_resource::project_read_resource_output(payload, TruncationPolicy::Bytes(8_000))
            .expect("project image resource");

    assert_eq!(
        output.body,
        vec![
            FunctionCallOutputContentItem::InputText {
                text: "MCP resource `image://example` from `hosted` (image/png)".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,aW1hZ2UtYnl0ZXM=".to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]
    );
}

#[test]
fn read_resource_projection_omits_raw_unhandled_binary_payload() {
    let payload = ReadResourcePayload {
        server: "hosted".to_string(),
        uri: "file://example.pdf".to_string(),
        result: ReadResourceResult::new(vec![ResourceContents::BlobResourceContents {
            uri: "file://example.pdf".to_string(),
            mime_type: Some("application/pdf".to_string()),
            blob: "cGRmLWJ5dGVz".to_string(),
            meta: None,
        }]),
    };

    let output =
        read_mcp_resource::project_read_resource_output(payload, TruncationPolicy::Bytes(8_000))
            .expect("project binary resource");

    assert_eq!(
        output.body,
        vec![
            FunctionCallOutputContentItem::InputText {
                text: "MCP resource `file://example.pdf` from `hosted` (application/pdf)"
                    .to_string(),
            },
            FunctionCallOutputContentItem::InputText {
                text: "[binary payload omitted: 12 base64 characters]".to_string(),
            },
        ]
    );
}
