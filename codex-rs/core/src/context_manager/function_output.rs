use super::history::estimate_function_output_content_item_tokens;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_utils_output_truncation::MAX_FUNCTION_OUTPUT_IMAGE_ENCODED_BYTES;
use codex_utils_output_truncation::MAX_FUNCTION_OUTPUT_IMAGES;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use std::sync::LazyLock;
use tiktoken_rs::CoreBPE;

static FUNCTION_OUTPUT_TOKENIZER: LazyLock<Option<CoreBPE>> =
    LazyLock::new(|| tiktoken_rs::o200k_base().ok());

pub(crate) fn truncate_function_output_payload(
    output: &FunctionCallOutputPayload,
    policy: TruncationPolicy,
) -> FunctionCallOutputPayload {
    let body = match &output.body {
        FunctionCallOutputBody::Text(text) => {
            FunctionCallOutputBody::Text(truncate_text_payload_to_fit(text, output.success, policy))
        }
        FunctionCallOutputBody::ContentItems(items) => FunctionCallOutputBody::ContentItems(
            truncate_content_items_to_fit(items, output.success, policy),
        ),
    };
    FunctionCallOutputPayload {
        body,
        success: output.success,
    }
}

pub(crate) fn function_output_payload_cost(
    output: &FunctionCallOutputPayload,
    policy: TruncationPolicy,
) -> usize {
    if matches!(policy, TruncationPolicy::Bytes(_)) {
        return serialized_cost(output, policy);
    }

    match &output.body {
        FunctionCallOutputBody::Text(_) => serialized_cost(output, policy),
        FunctionCallOutputBody::ContentItems(items) => {
            let mut media_tokens = 0usize;
            let normalized = items
                .iter()
                .map(|item| match item {
                    FunctionCallOutputContentItem::InputText { .. } => item.clone(),
                    FunctionCallOutputContentItem::InputImage { image_url, detail } => {
                        media_tokens = media_tokens.saturating_add(non_text_token_cost(item));
                        FunctionCallOutputContentItem::InputImage {
                            image_url: media_reference_prefix(image_url).to_string(),
                            detail: *detail,
                        }
                    }
                    FunctionCallOutputContentItem::InputAudio { audio_url } => {
                        media_tokens = media_tokens.saturating_add(non_text_token_cost(item));
                        FunctionCallOutputContentItem::InputAudio {
                            audio_url: media_reference_prefix(audio_url).to_string(),
                        }
                    }
                    FunctionCallOutputContentItem::EncryptedContent { .. } => {
                        media_tokens = media_tokens.saturating_add(non_text_token_cost(item));
                        FunctionCallOutputContentItem::EncryptedContent {
                            encrypted_content: String::new(),
                        }
                    }
                })
                .collect();
            let normalized = FunctionCallOutputPayload {
                body: FunctionCallOutputBody::ContentItems(normalized),
                success: output.success,
            };
            serialized_cost(&normalized, policy).saturating_add(media_tokens)
        }
    }
}

fn truncate_text_payload_to_fit(
    text: &str,
    success: Option<bool>,
    policy: TruncationPolicy,
) -> String {
    let budget = policy_budget(policy);
    let full = FunctionCallOutputPayload {
        body: FunctionCallOutputBody::Text(text.to_string()),
        success,
    };
    if function_output_payload_cost(&full, policy) <= budget {
        return text.to_string();
    }

    let mut lower = 0usize;
    let mut upper = candidate_budget_upper_bound(text, policy);
    let mut fitted = String::new();
    while lower <= upper {
        let candidate_budget = lower + (upper - lower) / 2;
        let candidate = truncate_text(text, candidate_policy(policy, candidate_budget));
        let payload = FunctionCallOutputPayload {
            body: FunctionCallOutputBody::Text(candidate.clone()),
            success,
        };
        if function_output_payload_cost(&payload, policy) <= budget {
            fitted = candidate;
            lower = candidate_budget.saturating_add(1);
        } else if candidate_budget == 0 {
            break;
        } else {
            upper = candidate_budget - 1;
        }
    }
    fitted
}

fn truncate_content_items_to_fit(
    items: &[FunctionCallOutputContentItem],
    success: Option<bool>,
    policy: TruncationPolicy,
) -> Vec<FunctionCallOutputContentItem> {
    let budget = policy_budget(policy);
    let empty = FunctionCallOutputPayload {
        body: FunctionCallOutputBody::ContentItems(Vec::new()),
        success,
    };
    let mut cost = function_output_payload_cost(&empty, policy);
    if cost > budget {
        return Vec::new();
    }

    let mut retained = Vec::new();
    let mut retained_images = 0usize;
    let mut retained_image_bytes = 0usize;
    let mut omitted = [0usize; 4];
    for item in items {
        if let FunctionCallOutputContentItem::InputImage { image_url, .. } = item
            && (retained_images >= MAX_FUNCTION_OUTPUT_IMAGES
                || retained_image_bytes.saturating_add(image_url.len())
                    > MAX_FUNCTION_OUTPUT_IMAGE_ENCODED_BYTES)
        {
            omitted[1] += 1;
            continue;
        }

        let separator_cost = separator_cost(retained.len(), policy);
        let item_cost = content_item_cost(item, policy);
        if cost
            .saturating_add(separator_cost)
            .saturating_add(item_cost)
            <= budget
        {
            if let FunctionCallOutputContentItem::InputImage { image_url, .. } = item {
                retained_images += 1;
                retained_image_bytes = retained_image_bytes.saturating_add(image_url.len());
            }
            retained.push(item.clone());
            cost = cost
                .saturating_add(separator_cost)
                .saturating_add(item_cost);
            continue;
        }

        match item {
            FunctionCallOutputContentItem::InputText { text } => {
                if let Some(truncated) = truncate_text_content_item_to_fit(
                    text,
                    budget.saturating_sub(cost.saturating_add(separator_cost)),
                    policy,
                ) {
                    cost = cost
                        .saturating_add(separator_cost)
                        .saturating_add(content_item_cost(&truncated, policy));
                    retained.push(truncated);
                }
                omitted[0] += 1;
            }
            FunctionCallOutputContentItem::InputImage { .. } => omitted[1] += 1,
            FunctionCallOutputContentItem::InputAudio { .. } => omitted[2] += 1,
            FunctionCallOutputContentItem::EncryptedContent { .. } => omitted[3] += 1,
        }
    }

    let omitted_labels = ["text", "image", "audio", "encrypted"];
    let omitted = omitted
        .into_iter()
        .zip(omitted_labels)
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect::<Vec<_>>();
    if !omitted.is_empty() {
        let marker = FunctionCallOutputContentItem::InputText {
            text: format!(
                "[omitted {} items to fit output budget]",
                omitted.join(", ")
            ),
        };
        let separator_cost = separator_cost(retained.len(), policy);
        let marker_cost = content_item_cost(&marker, policy);
        if cost
            .saturating_add(separator_cost)
            .saturating_add(marker_cost)
            <= budget
        {
            retained.push(marker);
        }
    }
    while function_output_payload_cost(
        &FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(retained.clone()),
            success,
        },
        policy,
    ) > budget
    {
        if retained.pop().is_none() {
            break;
        }
    }
    retained
}

fn truncate_text_content_item_to_fit(
    text: &str,
    available: usize,
    policy: TruncationPolicy,
) -> Option<FunctionCallOutputContentItem> {
    let mut lower = 0usize;
    let mut upper = candidate_budget_upper_bound(text, policy);
    let mut fitted = None;
    while lower <= upper {
        let candidate_budget = lower + (upper - lower) / 2;
        let candidate = FunctionCallOutputContentItem::InputText {
            text: truncate_text(text, candidate_policy(policy, candidate_budget)),
        };
        if content_item_cost(&candidate, policy) <= available {
            fitted = Some(candidate);
            lower = candidate_budget.saturating_add(1);
        } else if candidate_budget == 0 {
            break;
        } else {
            upper = candidate_budget - 1;
        }
    }
    fitted
}

fn content_item_cost(item: &FunctionCallOutputContentItem, policy: TruncationPolicy) -> usize {
    if matches!(policy, TruncationPolicy::Bytes(_)) {
        return serialized_cost(item, policy);
    }

    match item {
        FunctionCallOutputContentItem::InputText { .. } => serialized_cost(item, policy).max(1),
        FunctionCallOutputContentItem::InputImage { image_url, detail } => {
            let structural = FunctionCallOutputContentItem::InputImage {
                image_url: media_reference_prefix(image_url).to_string(),
                detail: *detail,
            };
            serialized_cost(&structural, policy)
                .saturating_add(non_text_token_cost(item))
                .max(1)
        }
        FunctionCallOutputContentItem::InputAudio { audio_url } => {
            let structural = FunctionCallOutputContentItem::InputAudio {
                audio_url: media_reference_prefix(audio_url).to_string(),
            };
            serialized_cost(&structural, policy)
                .saturating_add(non_text_token_cost(item))
                .max(1)
        }
        FunctionCallOutputContentItem::EncryptedContent { .. } => {
            let structural = FunctionCallOutputContentItem::EncryptedContent {
                encrypted_content: String::new(),
            };
            serialized_cost(&structural, policy)
                .saturating_add(non_text_token_cost(item))
                .max(1)
        }
    }
}

fn media_reference_prefix(reference: &str) -> &str {
    reference
        .find(',')
        .map_or(reference, |comma| &reference[..=comma])
}

fn non_text_token_cost(item: &FunctionCallOutputContentItem) -> usize {
    estimate_function_output_content_item_tokens(item).max(1)
}

fn separator_cost(index: usize, policy: TruncationPolicy) -> usize {
    if index == 0 {
        0
    } else {
        serialized_text_cost(",", policy)
    }
}

fn candidate_budget_upper_bound(text: &str, policy: TruncationPolicy) -> usize {
    match policy {
        TruncationPolicy::Bytes(_) => text.len(),
        TruncationPolicy::Tokens(tokens) => tokens,
    }
}

fn candidate_policy(policy: TruncationPolicy, budget: usize) -> TruncationPolicy {
    match policy {
        TruncationPolicy::Bytes(_) => TruncationPolicy::Bytes(budget),
        TruncationPolicy::Tokens(_) => TruncationPolicy::Tokens(budget),
    }
}

fn policy_budget(policy: TruncationPolicy) -> usize {
    match policy {
        TruncationPolicy::Bytes(bytes) => bytes,
        TruncationPolicy::Tokens(tokens) => tokens,
    }
}

fn serialized_cost(value: &impl serde::Serialize, policy: TruncationPolicy) -> usize {
    serde_json::to_string(value).map_or(usize::MAX, |serialized| {
        serialized_text_cost(&serialized, policy)
    })
}

fn serialized_text_cost(serialized: &str, policy: TruncationPolicy) -> usize {
    match policy {
        TruncationPolicy::Bytes(_) => serialized.len(),
        TruncationPolicy::Tokens(_) => FUNCTION_OUTPUT_TOKENIZER
            .as_ref()
            .map_or(serialized.len(), |tokenizer| {
                tokenizer.count_ordinary(serialized)
            }),
    }
}

#[cfg(test)]
#[path = "function_output_tests.rs"]
mod tests;
