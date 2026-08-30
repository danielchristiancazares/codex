use serde::Deserialize;
use serde_json::Value;

use super::DEFAULT_IMAGE_DETAIL;
use super::FunctionCallOutputContentItem;
use super::ImageDetail;

const CODEX_ENCRYPTED_CONTENT_META_KEY: &str = "codex/encryptedContent";
const CODEX_IMAGE_DETAIL_META_KEY: &str = "codex/imageDetail";

#[derive(Deserialize)]
#[serde(tag = "type")]
enum McpContent {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(rename = "_meta", default)]
        meta: Option<Value>,
    },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: Option<String>,
        #[serde(rename = "_meta", default)]
        meta: Option<Value>,
    },
    #[serde(rename = "audio")]
    Audio {
        data: String,
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: Option<String>,
    },
    #[serde(rename = "resource")]
    Resource {
        resource: EmbeddedResourceContents,
        #[serde(rename = "_meta", default)]
        meta: Option<Value>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EmbeddedResourceContents {
    Text {
        uri: String,
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: Option<String>,
        text: String,
    },
    Blob {
        uri: String,
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: Option<String>,
        blob: String,
        #[serde(rename = "_meta", default)]
        meta: Option<Value>,
    },
}

pub(super) fn convert_mcp_content_to_items(
    contents: &[Value],
) -> Vec<FunctionCallOutputContentItem> {
    let mut items = Vec::with_capacity(contents.len());
    for content in contents {
        match serde_json::from_value::<McpContent>(content.clone()) {
            Ok(McpContent::Text { text, meta }) => {
                if meta
                    .as_ref()
                    .and_then(|meta| meta.get(CODEX_ENCRYPTED_CONTENT_META_KEY))
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    items.push(FunctionCallOutputContentItem::EncryptedContent {
                        encrypted_content: text,
                    });
                } else {
                    items.push(FunctionCallOutputContentItem::InputText { text });
                }
            }
            Ok(McpContent::Image {
                data,
                mime_type,
                meta,
            }) => items.push(FunctionCallOutputContentItem::InputImage {
                image_url: media_data_url(data, mime_type.as_deref()),
                detail: image_detail(meta.as_ref()),
            }),
            Ok(McpContent::Audio { data, mime_type }) => {
                items.push(FunctionCallOutputContentItem::InputAudio {
                    audio_url: media_data_url(data, mime_type.as_deref()),
                });
            }
            Ok(McpContent::Resource { resource, meta }) => {
                items.extend(embedded_resource_items(resource, meta.as_ref()));
            }
            Ok(McpContent::Unknown) => {
                items.push(FunctionCallOutputContentItem::InputText {
                    text: serde_json::to_string(content)
                        .unwrap_or_else(|_| "<content>".to_string()),
                });
            }
            Err(_) if content.get("type").and_then(Value::as_str) == Some("resource") => {
                items.push(FunctionCallOutputContentItem::InputText {
                    text: "MCP embedded resource uses an unsupported content type".to_string(),
                });
            }
            Err(_) => items.push(FunctionCallOutputContentItem::InputText {
                text: serde_json::to_string(content).unwrap_or_else(|_| "<content>".to_string()),
            }),
        }
    }
    items
}

fn embedded_resource_items(
    resource: EmbeddedResourceContents,
    outer_meta: Option<&Value>,
) -> Vec<FunctionCallOutputContentItem> {
    match resource {
        EmbeddedResourceContents::Text {
            uri,
            mime_type,
            text,
        } => vec![FunctionCallOutputContentItem::InputText {
            text: format!("{}\n{text}", resource_header(&uri, mime_type.as_deref())),
        }],
        EmbeddedResourceContents::Blob {
            uri,
            mime_type,
            blob,
            meta,
        } => {
            let normalized_mime_type = normalized_mime_type(mime_type.as_deref());
            let mut items = vec![FunctionCallOutputContentItem::InputText {
                text: resource_header(&uri, mime_type.as_deref()),
            }];
            match normalized_mime_type {
                Some(mime_type) if media_kind_is(mime_type, "image") => {
                    items.push(FunctionCallOutputContentItem::InputImage {
                        image_url: media_data_url(blob, Some(mime_type)),
                        detail: explicit_image_detail(meta.as_ref())
                            .or_else(|| explicit_image_detail(outer_meta))
                            .or(Some(DEFAULT_IMAGE_DETAIL)),
                    });
                }
                Some(mime_type) if media_kind_is(mime_type, "audio") => {
                    items.push(FunctionCallOutputContentItem::InputAudio {
                        audio_url: media_data_url(blob, Some(mime_type)),
                    });
                }
                _ => items.push(FunctionCallOutputContentItem::InputText {
                    text: format!("[binary payload omitted: {} base64 characters]", blob.len()),
                }),
            }
            items
        }
    }
}

fn resource_header(uri: &str, mime_type: Option<&str>) -> String {
    match mime_type {
        Some(mime_type) => format!("MCP embedded resource `{uri}` ({mime_type})"),
        None => format!("MCP embedded resource `{uri}`"),
    }
}

fn normalized_mime_type(mime_type: Option<&str>) -> Option<&str> {
    mime_type
        .and_then(|mime_type| mime_type.split(';').next())
        .map(str::trim)
        .filter(|mime_type| !mime_type.is_empty())
}

fn media_kind_is(mime_type: &str, expected: &str) -> bool {
    mime_type
        .split_once('/')
        .is_some_and(|(kind, _)| kind.eq_ignore_ascii_case(expected))
}

fn media_data_url(data: String, mime_type: Option<&str>) -> String {
    if data.starts_with("data:") {
        data
    } else {
        format!(
            "data:{};base64,{data}",
            mime_type.unwrap_or("application/octet-stream")
        )
    }
}

fn image_detail(meta: Option<&Value>) -> Option<ImageDetail> {
    explicit_image_detail(meta).or(Some(DEFAULT_IMAGE_DETAIL))
}

fn explicit_image_detail(meta: Option<&Value>) -> Option<ImageDetail> {
    meta.and_then(Value::as_object)
        .and_then(|meta| meta.get(CODEX_IMAGE_DETAIL_META_KEY))
        .and_then(Value::as_str)
        .and_then(|detail| match detail {
            "auto" => Some(ImageDetail::Auto),
            "low" => Some(ImageDetail::Low),
            "high" => Some(ImageDetail::High),
            "original" => Some(ImageDetail::Original),
            _ => None,
        })
}

#[cfg(test)]
#[path = "mcp_content_tests.rs"]
mod tests;
