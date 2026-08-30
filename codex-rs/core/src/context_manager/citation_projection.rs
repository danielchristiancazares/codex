use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_stream_parser::strip_citations;

pub(super) fn strip_hidden_citations(items: &mut [ResponseItemEnvelope]) {
    for envelope in items {
        let ResponseItem::Message { role, content, .. } = &mut envelope.item else {
            continue;
        };
        if role != "assistant" {
            continue;
        }
        for content_item in content {
            if let ContentItem::OutputText { text } = content_item {
                *text = strip_citations(text).0;
            }
        }
    }
}

#[cfg(test)]
#[path = "citation_projection_tests.rs"]
mod tests;
