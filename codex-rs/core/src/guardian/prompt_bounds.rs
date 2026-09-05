//! Bounds the complete text context sent in one synchronous Guardian review.

use std::ops::Range;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::user_input::UserInput;

pub(super) const MAX_ASSEMBLED_GUARDIAN_CONTEXT_TOKENS: usize = 10_000;
const OMISSION_NOTICE: &str = "Some conversation evidence was omitted to keep the complete Guardian context within the safety limit.\n";
const REQUIRED_CONTEXT_ERROR: &str = "Guardian authorization, required transcript anchors, and approval request exceed the 10000-token safety limit";

pub(super) fn bound_guardian_prompt_items(
    mut items: Vec<UserInput>,
    transcript_entries: Range<usize>,
    required_entry_indices: &[usize],
) -> anyhow::Result<Vec<UserInput>> {
    let transcript_entry_count = transcript_entries
        .end
        .checked_sub(transcript_entries.start)
        .filter(|_| transcript_entries.end <= items.len())
        .ok_or_else(|| anyhow::anyhow!("Guardian transcript retention metadata is invalid"))?;
    let mut required = required_entry_mask(transcript_entry_count, required_entry_indices)?;
    let max_bytes = TruncationPolicy::Tokens(MAX_ASSEMBLED_GUARDIAN_CONTEXT_TOKENS).byte_budget();
    if serialized_text_bytes(&items)? <= max_bytes {
        return Ok(items);
    }
    if transcript_entries.is_empty() {
        anyhow::bail!("{REQUIRED_CONTEXT_ERROR}");
    }

    items.insert(
        transcript_entries.start,
        UserInput::Text {
            text: OMISSION_NOTICE.to_string(),
            text_elements: Vec::new(),
        },
    );
    while serialized_text_bytes(&items)? > max_bytes {
        let Some(oldest_optional) = required.iter().position(|required| !required) else {
            anyhow::bail!("{REQUIRED_CONTEXT_ERROR}");
        };
        items.remove(transcript_entries.start + 1 + oldest_optional);
        required.remove(oldest_optional);
    }
    Ok(items)
}

fn required_entry_mask(
    entry_count: usize,
    required_entry_indices: &[usize],
) -> anyhow::Result<Vec<bool>> {
    let mut required = vec![false; entry_count];
    for &index in required_entry_indices {
        let Some(slot) = required.get_mut(index) else {
            anyhow::bail!("Guardian transcript retention metadata is invalid");
        };
        if *slot {
            anyhow::bail!("Guardian transcript retention metadata is invalid");
        }
        *slot = true;
    }
    Ok(required)
}

fn serialized_text_bytes(items: &[UserInput]) -> anyhow::Result<usize> {
    let content = items
        .iter()
        .filter_map(|item| match item {
            UserInput::Text { text, .. } => Some(ContentItem::InputText { text: text.clone() }),
            UserInput::Image { .. }
            | UserInput::LocalImage { .. }
            | UserInput::Audio { .. }
            | UserInput::LocalAudio { .. }
            | UserInput::Skill { .. }
            | UserInput::Mention { .. } => None,
            _ => None,
        })
        .collect();
    Ok(serde_json::to_vec(&ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })?
    .len())
}

#[cfg(test)]
#[path = "prompt_bounds_tests.rs"]
mod tests;
