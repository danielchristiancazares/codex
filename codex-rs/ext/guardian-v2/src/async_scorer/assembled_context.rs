//! Applies the fork-wide bound to the complete model-visible Guardian context item.

use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::user_input::UserInput;

use super::action::GuardianAction;
use super::action::RenderedAction;

pub(super) const MAX_ASSEMBLED_CONTEXT_TOKENS: usize = 10_000;
const OMISSION_NOTICE: &str = "Some conversation evidence was omitted to keep the complete Guardian context within the safety limit.\n";
const REQUIRED_CONTEXT_ERROR: &str = "Guardian authorization, required transcript anchors, and approval request exceed the 10000-token safety limit";

pub(super) struct BoundedModelContext {
    pub(super) items: Vec<String>,
    pub(super) original_bytes: usize,
    pub(super) retained_bytes: usize,
}

pub(super) struct PreparedModelContext {
    pub(super) items: Vec<String>,
    pub(super) action: RenderedAction,
    pub(super) original_bytes: usize,
    pub(super) retained_bytes: usize,
}

pub(super) fn prepare_model_context(
    mut authorization: Vec<String>,
    entries: Vec<String>,
    required_entry_indices: Vec<usize>,
    action: GuardianAction,
    max_action_tokens: usize,
) -> Result<PreparedModelContext, String> {
    authorization.push(">>> TRANSCRIPT START\n".to_owned());
    let required = required_entry_mask(entries.len(), &required_entry_indices)?;
    let has_optional_entries = required.iter().any(|required| !required);
    let required_entries = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| required[index].then_some(entry))
        .collect::<Vec<_>>();
    let placeholder_suffix = approval_suffix("\n".to_owned());
    let reserved_items = assemble(
        &authorization,
        has_optional_entries.then_some(OMISSION_NOTICE),
        required_entries,
        &placeholder_suffix,
    );
    let max_bytes = TruncationPolicy::Tokens(MAX_ASSEMBLED_CONTEXT_TOKENS).byte_budget();
    let reserved_bytes = serialized_bytes(&reserved_items)?;
    if reserved_bytes > max_bytes {
        return Err(REQUIRED_CONTEXT_ERROR.to_owned());
    }
    let placeholder_bytes = serialized_text_item_bytes("\n")?;
    let fixed_bytes = reserved_bytes
        .checked_sub(placeholder_bytes)
        .ok_or_else(|| {
            "failed to measure Guardian action framing within the complete context".to_owned()
        })?;
    let max_serialized_action_text_bytes = max_bytes.checked_sub(fixed_bytes).ok_or_else(|| {
        "failed to reserve Guardian action capacity within the complete context".to_owned()
    })?;
    let action = action
        .render_for_context(max_action_tokens, max_serialized_action_text_bytes)
        .map_err(|error| format!("Guardian action serialization failed: {error}"))?;
    let suffix = approval_suffix(format!("{}\n", action.text));
    let bounded = bound_model_context_with_mask(authorization, entries, required, suffix)?;

    Ok(PreparedModelContext {
        items: bounded.items,
        action,
        original_bytes: bounded.original_bytes,
        retained_bytes: bounded.retained_bytes,
    })
}

#[cfg(test)]
pub(super) fn bound_model_context(
    prefix: Vec<String>,
    entries: Vec<String>,
    required_entry_indices: Vec<usize>,
    suffix: Vec<String>,
) -> Result<BoundedModelContext, String> {
    let required = required_entry_mask(entries.len(), &required_entry_indices)?;
    bound_model_context_with_mask(prefix, entries, required, suffix)
}

fn bound_model_context_with_mask(
    prefix: Vec<String>,
    entries: Vec<String>,
    mut required: Vec<bool>,
    suffix: Vec<String>,
) -> Result<BoundedModelContext, String> {
    let original = assemble(&prefix, None, entries.iter(), &suffix);
    let original_bytes = serialized_bytes(&original)?;
    let max_bytes = TruncationPolicy::Tokens(MAX_ASSEMBLED_CONTEXT_TOKENS).byte_budget();
    if original_bytes <= max_bytes {
        return Ok(BoundedModelContext {
            items: original,
            original_bytes,
            retained_bytes: original_bytes,
        });
    }

    let mut retained = entries;
    loop {
        let items = assemble(&prefix, Some(OMISSION_NOTICE), retained.iter(), &suffix);
        let retained_bytes = serialized_bytes(&items)?;
        if retained_bytes <= max_bytes {
            return Ok(BoundedModelContext {
                items,
                original_bytes,
                retained_bytes,
            });
        }
        let oldest_optional = oldest_optional_index(&required)?;
        retained.remove(oldest_optional);
        required.remove(oldest_optional);
    }
}

pub(crate) fn bound_user_inputs(
    mut items: Vec<UserInput>,
    transcript_entries: std::ops::Range<usize>,
    required_entry_indices: &[usize],
) -> Result<Vec<UserInput>, String> {
    let transcript_entry_count = transcript_entries
        .end
        .checked_sub(transcript_entries.start)
        .filter(|_| transcript_entries.end <= items.len())
        .ok_or_else(|| "Guardian transcript retention metadata is invalid".to_owned())?;
    let mut required = required_entry_mask(transcript_entry_count, required_entry_indices)?;
    let max_bytes = TruncationPolicy::Tokens(MAX_ASSEMBLED_CONTEXT_TOKENS).byte_budget();
    if serialized_user_input_bytes(&items)? <= max_bytes {
        return Ok(items);
    }
    if transcript_entries.is_empty() {
        return Err(REQUIRED_CONTEXT_ERROR.to_owned());
    }

    items.insert(
        transcript_entries.start,
        UserInput::Text {
            text: OMISSION_NOTICE.to_string(),
            text_elements: Vec::new(),
        },
    );
    while serialized_user_input_bytes(&items)? > max_bytes {
        let oldest_optional = oldest_optional_index(&required)?;
        items.remove(transcript_entries.start + 1 + oldest_optional);
        required.remove(oldest_optional);
    }
    Ok(items)
}

fn required_entry_mask(
    entry_count: usize,
    required_entry_indices: &[usize],
) -> Result<Vec<bool>, String> {
    let mut required = vec![false; entry_count];
    for &index in required_entry_indices {
        let Some(slot) = required.get_mut(index) else {
            return Err("Guardian transcript retention metadata is invalid".to_owned());
        };
        if *slot {
            return Err("Guardian transcript retention metadata is invalid".to_owned());
        }
        *slot = true;
    }
    Ok(required)
}

fn oldest_optional_index(required: &[bool]) -> Result<usize, String> {
    required
        .iter()
        .position(|required| !required)
        .ok_or_else(|| REQUIRED_CONTEXT_ERROR.to_owned())
}

fn approval_suffix(action_text: String) -> Vec<String> {
    vec![
        ">>> TRANSCRIPT END\n\n".to_owned(),
        "The Codex agent has requested the following action:\n".to_owned(),
        ">>> APPROVAL REQUEST START\n".to_owned(),
        "Planned action JSON:\n".to_owned(),
        action_text,
        ">>> APPROVAL REQUEST END\n".to_owned(),
    ]
}

fn assemble<'a>(
    prefix: &[String],
    notice: Option<&str>,
    entries: impl IntoIterator<Item = &'a String>,
    suffix: &[String],
) -> Vec<String> {
    prefix
        .iter()
        .cloned()
        .chain(notice.map(str::to_string))
        .chain(entries.into_iter().cloned())
        .chain(suffix.iter().cloned())
        .collect()
}

fn serialized_bytes(items: &[String]) -> Result<usize, String> {
    serialized_text_bytes(items.iter().cloned())
}

fn serialized_text_item_bytes(text: &str) -> Result<usize, String> {
    serde_json::to_vec(&ContentItem::InputText {
        text: text.to_owned(),
    })
    .map(|encoded| encoded.len())
    .map_err(|error| format!("failed to measure Guardian context: {error}"))
}

fn serialized_user_input_bytes(items: &[UserInput]) -> Result<usize, String> {
    serialized_text_bytes(items.iter().filter_map(|item| match item {
        UserInput::Text { text, .. } => Some(text.clone()),
        UserInput::Image { .. }
        | UserInput::LocalImage { .. }
        | UserInput::Audio { .. }
        | UserInput::LocalAudio { .. }
        | UserInput::Skill { .. }
        | UserInput::Mention { .. } => None,
        _ => None,
    }))
}

fn serialized_text_bytes(texts: impl IntoIterator<Item = String>) -> Result<usize, String> {
    serde_json::to_vec(&ResponseItem::Message {
        // The sampler adds a msg-prefixed UUID after assembly; budget its exact wire shape.
        id: Some(ResponseItemId::with_suffix("msg", uuid::Uuid::nil())),
        role: "user".to_string(),
        content: texts
            .into_iter()
            .map(|text| ContentItem::InputText { text })
            .collect(),
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })
    .map(|encoded| encoded.len())
    .map_err(|error| format!("failed to measure Guardian context: {error}"))
}

#[cfg(test)]
#[path = "assembled_context_tests.rs"]
mod tests;
