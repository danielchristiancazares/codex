use codex_protocol::error::CodexErr;
use codex_protocol::models::AgentMessageInputContent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_rollout::RolloutItem;
use codex_rollout::should_persist_response_item_for_memories;
use codex_secrets::redact_secrets;
use codex_utils_stream_parser::strip_citations;
use serde_json::Value;

/// Serializes the bounded, text-oriented rollout projection used by memory Phase 1.
pub(crate) fn serialize_filtered_rollout_response_items(
    items: &[RolloutItem],
) -> codex_protocol::error::Result<String> {
    let checkpoint = items.iter().rposition(
        |item| matches!(item, RolloutItem::Compacted(compacted) if compacted.replacement_history.is_some()),
    );
    let checkpoint_history = checkpoint
        .and_then(|index| match &items[index] {
            RolloutItem::Compacted(compacted) => compacted.replacement_history.as_deref(),
            _ => None,
        })
        .unwrap_or_default();
    let suffix = checkpoint.map_or(items, |index| &items[index + 1..]);
    let filtered = checkpoint_history
        .iter()
        .filter_map(|item| sanitize_response_item(&item.item))
        .chain(suffix.iter().filter_map(|item| match item {
            RolloutItem::ResponseItem(item) => sanitize_response_item(&item.item),
            RolloutItem::InterAgentCommunication(communication) => {
                Some(communication.to_model_input_item())
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::TokenUsageRecord(_)
            | RolloutItem::RealtimeItem(_)
            | RolloutItem::WorldState(_)
            | RolloutItem::RetainedContext(_)
            | RolloutItem::SecurityRiskScore(_)
            | RolloutItem::EventMsg(_) => None,
        }))
        .collect::<Vec<_>>();
    let serialized = serde_json::to_string(&filtered).map_err(|err| {
        CodexErr::InvalidRequest(format!("failed to serialize rollout memory: {err}"))
    })?;
    Ok(redact_secrets(serialized))
}

fn sanitize_response_item(item: &ResponseItem) -> Option<ResponseItem> {
    if !should_persist_response_item_for_memories(item) {
        return None;
    }

    let mut item = item.clone();
    match &mut item {
        ResponseItem::Message { role, content, .. } => {
            if role == "developer" {
                return None;
            }

            *content = content
                .iter()
                .filter(|content_item| {
                    role != "user" || !is_memory_excluded_contextual_user_fragment(content_item)
                })
                .filter_map(sanitize_content_item)
                .collect();
            if content.is_empty() {
                return None;
            }
        }
        ResponseItem::AgentMessage { content, .. } => {
            for content_item in content {
                if let AgentMessageInputContent::InputText { text } = content_item {
                    *text = sanitize_text(text);
                }
            }
        }
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => {
            sanitize_function_call_output(output);
        }
        ResponseItem::AdditionalTools { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::ConfigurationUpdate { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger {}
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => {}
    }

    Some(item)
}

fn sanitize_content_item(item: &ContentItem) -> Option<ContentItem> {
    match item {
        ContentItem::InputText { text } => {
            let text = sanitize_text(text);
            (!text.trim().is_empty()).then_some(ContentItem::InputText { text })
        }
        ContentItem::OutputText { text } => {
            let text = sanitize_text(text);
            (!text.trim().is_empty()).then_some(ContentItem::OutputText { text })
        }
        ContentItem::InputImage { image_url, .. } => Some(ContentItem::InputText {
            text: media_placeholder(MediaKind::Image, data_url_media_type(image_url)),
        }),
        ContentItem::InputAudio { audio_url } => Some(ContentItem::InputText {
            text: media_placeholder(MediaKind::Audio, data_url_media_type(audio_url)),
        }),
    }
}

fn sanitize_function_call_output(output: &mut FunctionCallOutputPayload) {
    match &mut output.body {
        FunctionCallOutputBody::Text(text) => {
            *text = sanitize_text(text);
        }
        FunctionCallOutputBody::ContentItems(items) => {
            for item in items {
                *item = match item {
                    FunctionCallOutputContentItem::InputText { text } => {
                        FunctionCallOutputContentItem::InputText {
                            text: sanitize_text(text),
                        }
                    }
                    FunctionCallOutputContentItem::InputImage { image_url, .. } => {
                        FunctionCallOutputContentItem::InputText {
                            text: media_placeholder(
                                MediaKind::Image,
                                data_url_media_type(image_url),
                            ),
                        }
                    }
                    FunctionCallOutputContentItem::InputAudio { audio_url } => {
                        FunctionCallOutputContentItem::InputText {
                            text: media_placeholder(
                                MediaKind::Audio,
                                data_url_media_type(audio_url),
                            ),
                        }
                    }
                    FunctionCallOutputContentItem::EncryptedContent { .. } => item.clone(),
                };
            }
        }
    }
}

fn sanitize_text(text: &str) -> String {
    let (text, _) = strip_citations(text);
    let text = strip_tagged_blocks(&text, "<memory_citation>", "</memory_citation>");
    let text = replace_inline_media_data_urls(&text);

    let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
        return text;
    };
    if !sanitize_json_media(&mut value) {
        return text;
    }
    serde_json::to_string(&value).unwrap_or(text)
}

fn strip_tagged_blocks(text: &str, start_marker: &str, end_marker: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(start) = remaining.find(start_marker) {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + start_marker.len()..];
        let Some(end) = after_start.find(end_marker) else {
            return output;
        };
        remaining = &after_start[end + end_marker.len()..];
    }
    output.push_str(remaining);
    output
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MediaKind {
    Image,
    Audio,
}

impl MediaKind {
    fn label(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Audio => "Audio",
        }
    }

    fn matches_media_type(self, media_type: &str) -> bool {
        match self {
            Self::Image => media_type.starts_with("image/"),
            Self::Audio => media_type.starts_with("audio/"),
        }
    }
}

fn media_placeholder(kind: MediaKind, media_type: Option<&str>) -> String {
    let label = kind.label();
    match media_type
        .filter(|media_type| kind.matches_media_type(media_type) && valid_media_type(media_type))
    {
        Some(media_type) => format!("[{label}: {media_type}]"),
        None => format!("[{label}]"),
    }
}

fn valid_media_type(media_type: &str) -> bool {
    !media_type.is_empty()
        && media_type.len() <= 64
        && media_type
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'+' | b'-' | b'.'))
}

fn data_url_media_type(url: &str) -> Option<&str> {
    let header = url.strip_prefix("data:")?.split_once(',')?.0;
    header.split(';').next()
}

fn replace_inline_media_data_urls(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some((start, kind)) = next_media_data_url(remaining) {
        output.push_str(&remaining[..start]);
        let candidate = &remaining[start..];
        let Some(comma) = candidate.find(',') else {
            output.push_str(candidate);
            return output;
        };
        let header = &candidate[..comma];
        if !header
            .split(';')
            .any(|part| part.eq_ignore_ascii_case("base64"))
        {
            output.push(candidate.as_bytes()[0] as char);
            remaining = &candidate[1..];
            continue;
        }

        let payload = &candidate[comma + 1..];
        let payload_len = payload
            .bytes()
            .take_while(|byte| {
                byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/' | b'=' | b'-' | b'_')
            })
            .count();
        if payload_len == 0 {
            output.push(candidate.as_bytes()[0] as char);
            remaining = &candidate[1..];
            continue;
        }

        output.push_str(&media_placeholder(kind, data_url_media_type(candidate)));
        remaining = &payload[payload_len..];
    }
    output.push_str(remaining);
    output
}

fn next_media_data_url(text: &str) -> Option<(usize, MediaKind)> {
    match (text.find("data:image/"), text.find("data:audio/")) {
        (Some(image), Some(audio)) if image <= audio => Some((image, MediaKind::Image)),
        (Some(_), Some(audio)) => Some((audio, MediaKind::Audio)),
        (Some(image), None) => Some((image, MediaKind::Image)),
        (None, Some(audio)) => Some((audio, MediaKind::Audio)),
        (None, None) => None,
    }
}

fn sanitize_json_media(value: &mut Value) -> bool {
    match value {
        Value::Array(values) => {
            let mut changed = false;
            for value in values {
                changed |= sanitize_json_media(value);
            }
            changed
        }
        Value::Object(values) => {
            let media = json_object_media_kind(values);
            let media_type = json_object_media_type(values).map(str::to_string);
            let mut changed = false;
            for (key, value) in values {
                let keyed_media = match key.as_str() {
                    "image_url" | "imageUrl" => Some(MediaKind::Image),
                    "audio_url" | "audioUrl" => Some(MediaKind::Audio),
                    "data" | "blob" => media,
                    _ => None,
                };
                if let (Some(kind), Value::String(content)) = (keyed_media, &*value) {
                    let replacement = media_placeholder(
                        kind,
                        data_url_media_type(content).or_else(|| {
                            media_type
                                .as_deref()
                                .filter(|media_type| kind.matches_media_type(media_type))
                        }),
                    );
                    if replacement != *content {
                        *value = Value::String(replacement);
                        changed = true;
                    }
                } else {
                    changed |= sanitize_json_media(value);
                }
            }
            changed
        }
        Value::String(text) => {
            let replacement = replace_inline_media_data_urls(text);
            if replacement == *text {
                false
            } else {
                *text = replacement;
                true
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn json_object_media_kind(values: &serde_json::Map<String, Value>) -> Option<MediaKind> {
    if let Some(media_type) = json_object_media_type(values) {
        if media_type.starts_with("image/") {
            return Some(MediaKind::Image);
        }
        if media_type.starts_with("audio/") {
            return Some(MediaKind::Audio);
        }
    }
    match values.get("type").and_then(Value::as_str) {
        Some("image" | "input_image") => Some(MediaKind::Image),
        Some("audio" | "input_audio") => Some(MediaKind::Audio),
        Some(_) | None => None,
    }
}

fn json_object_media_type(values: &serde_json::Map<String, Value>) -> Option<&str> {
    ["mimeType", "mime_type", "mediaType", "media_type"]
        .into_iter()
        .filter_map(|key| values.get(key).and_then(Value::as_str))
        .find(|media_type| media_type.starts_with("image/") || media_type.starts_with("audio/"))
}

pub(crate) fn is_memory_excluded_contextual_user_fragment(content_item: &ContentItem) -> bool {
    let ContentItem::InputText { text } = content_item else {
        return false;
    };

    matches_marked_fragment(text, "# AGENTS.md instructions", "</INSTRUCTIONS>")
        || matches_marked_fragment(text, "<skill>", "</skill>")
}

fn matches_marked_fragment(text: &str, start_marker: &str, end_marker: &str) -> bool {
    let trimmed = text.trim_start();
    let starts_with_marker = trimmed
        .get(..start_marker.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(start_marker));
    let trimmed = trimmed.trim_end();
    let ends_with_marker = trimmed
        .get(trimmed.len().saturating_sub(end_marker.len())..)
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(end_marker));
    starts_with_marker && ends_with_marker
}
