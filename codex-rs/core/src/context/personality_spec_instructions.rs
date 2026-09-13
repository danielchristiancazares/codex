use super::ContextualUserFragment;
use codex_guardian_context::truncate_text;
use codex_protocol::models::ContentItemKind;
use codex_utils_string::approx_bytes_for_tokens;

pub(super) const MAX_PERSONALITY_SPEC_INSTRUCTIONS_TOKENS: usize = 1_000;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PersonalitySpecInstructions {
    spec: String,
}

impl PersonalitySpecInstructions {
    pub(crate) fn new(spec: impl Into<String>) -> Self {
        let spec = spec.into();
        let framing_bytes = Self {
            spec: String::new(),
        }
        .render()
        .len();
        let spec_tokens = approx_bytes_for_tokens(MAX_PERSONALITY_SPEC_INSTRUCTIONS_TOKENS)
            .saturating_sub(framing_bytes)
            / 4;
        Self {
            spec: truncate_text(&spec, spec_tokens),
        }
    }
}

impl ContextualUserFragment for PersonalitySpecInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("personality.spec_instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<personality_spec>", "</personality_spec>")
    }

    fn body(&self) -> String {
        format!(
            " The user has requested a new communication style. Future messages should adhere to the following personality: \n{} ",
            self.spec
        )
    }
}
