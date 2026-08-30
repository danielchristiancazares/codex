use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn embedded_image_resource_becomes_typed_image_content() {
    let items = convert_mcp_content_to_items(&[json!({
        "type": "resource",
        "resource": {
            "uri": "image://example",
            "mimeType": "image/png",
            "blob": "aW1hZ2U=",
        },
    })]);

    assert_eq!(
        items,
        vec![
            FunctionCallOutputContentItem::InputText {
                text: "MCP embedded resource `image://example` (image/png)".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,aW1hZ2U=".to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]
    );
}

#[test]
fn embedded_text_resource_preserves_uri_and_text() {
    let items = convert_mcp_content_to_items(&[json!({
        "type": "resource",
        "resource": {
            "uri": "memo://example",
            "mimeType": "text/plain",
            "text": "remember this",
        },
    })]);

    assert_eq!(
        items,
        vec![FunctionCallOutputContentItem::InputText {
            text: "MCP embedded resource `memo://example` (text/plain)\nremember this".to_string(),
        }]
    );
}

#[test]
fn embedded_unhandled_blob_omits_raw_base64() {
    let items = convert_mcp_content_to_items(&[json!({
        "type": "resource",
        "resource": {
            "uri": "file://example.pdf",
            "mimeType": "application/pdf",
            "blob": "cGRmLWJ5dGVz",
        },
    })]);

    assert_eq!(
        items,
        vec![
            FunctionCallOutputContentItem::InputText {
                text: "MCP embedded resource `file://example.pdf` (application/pdf)".to_string(),
            },
            FunctionCallOutputContentItem::InputText {
                text: "[binary payload omitted: 12 base64 characters]".to_string(),
            },
        ]
    );
}
