use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;

const COMPACTION_SUMMARY_CONTENT_KIND: &str = "compaction.summary";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompactionSummary {
    summary: String,
}

impl CompactionSummary {
    pub(crate) fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
        }
    }

    pub(crate) fn is_summary_item(item: &ResponseItem) -> bool {
        matches!(
            item,
            ResponseItem::Message {
                role,
                content,
                internal_chat_message_metadata_passthrough: Some(metadata),
                ..
            } if role == "user"
                && content.len() == 1
                && matches!(
                    metadata.content_item_kinds.as_deref(),
                    Some([kind]) if kind.0 == COMPACTION_SUMMARY_CONTENT_KIND
                )
        )
    }
}

impl ContextualUserFragment for CompactionSummary {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind(COMPACTION_SUMMARY_CONTENT_KIND.to_string())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        self.summary.clone()
    }
}
