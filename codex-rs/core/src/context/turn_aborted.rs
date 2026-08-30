use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ResponseItem;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TurnAborted {
    pub(crate) guidance: String,
}

impl TurnAborted {
    pub(crate) const INTERRUPTED_GUIDANCE: &'static str = "The user interrupted the previous turn on purpose. Any running unified exec processes may still be running in the background. If any tools/commands were aborted, they may have partially executed.";
    pub(crate) const INTERRUPTED_DEVELOPER_GUIDANCE: &'static str = "The previous turn was interrupted on purpose. Any running unified exec processes may still be running in the background. If any tools/commands were aborted, they may have partially executed.";
    pub(crate) const RESUMED_GUIDANCE: &'static str = "The user interrupted the previous turn on purpose. Unified exec process handles from before this resumed session are terminated and their session IDs are retired. If any tools/commands were aborted, they may have partially executed.";
    pub(crate) const RESUMED_DEVELOPER_GUIDANCE: &'static str = "The previous turn was interrupted on purpose. Unified exec process handles from before this resumed session are terminated and their session IDs are retired. If any tools/commands were aborted, they may have partially executed.";

    pub(crate) fn new(guidance: impl Into<String>) -> Self {
        Self {
            guidance: guidance.into(),
        }
    }

    pub(crate) fn rewrite_response_item_for_resume(item: &mut ResponseItem) {
        let ResponseItem::Message { role, content, .. } = item else {
            return;
        };
        let resumed = match role.as_str() {
            "user" => Self::new(Self::RESUMED_GUIDANCE).render(),
            "developer" => Self::new(Self::RESUMED_DEVELOPER_GUIDANCE).render(),
            _ => return,
        };
        for content_item in content {
            let ContentItem::InputText { text } = content_item else {
                continue;
            };
            if Self::matches_text(text)
                && (text.contains(Self::INTERRUPTED_GUIDANCE)
                    || text.contains(Self::INTERRUPTED_DEVELOPER_GUIDANCE))
            {
                *text = resumed.clone();
            }
        }
    }
}

impl ContextualUserFragment for TurnAborted {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("generic.turn_aborted".to_string())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<turn_aborted>", "</turn_aborted>")
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.guidance)
    }
}

#[cfg(test)]
#[path = "turn_aborted_tests.rs"]
mod tests;
