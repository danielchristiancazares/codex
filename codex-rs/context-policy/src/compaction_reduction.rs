//! Selects removable turn groups and complete tool exchanges for local compaction.

use codex_protocol::models::ResponseItem;

use crate::RequestBudget;

/// An item measured and classified by the history owner.
pub struct CompactionItem<'a> {
    pub item: &'a ResponseItem,
    pub estimated_tokens: i64,
    pub starts_turn: bool,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("local compaction cannot reduce history further without dropping the newest turn")]
pub struct CompactionReductionExhausted;

#[derive(Debug, PartialEq, Eq)]
pub struct LocalCompactionReduction {
    pub removed_groups: usize,
    pub removed_items: usize,
    pub estimated_tokens_before: i64,
    pub estimated_tokens_after: i64,
    pub target_tokens: i64,
}

/// A selection the history owner can apply while preserving its item envelopes.
#[derive(Debug, PartialEq, Eq)]
pub struct CompactionReduction {
    pub retained_indices: Vec<usize>,
    pub reduction: LocalCompactionReduction,
}

pub fn plan_compaction_reduction(
    items: &[CompactionItem<'_>],
    compaction_input_items: usize,
    base_tokens: i64,
    budget: RequestBudget,
) -> Result<CompactionReduction, CompactionReductionExhausted> {
    if items.len() <= compaction_input_items {
        return Err(CompactionReductionExhausted);
    }
    // The summarization instruction belongs to the newest real turn group.
    let retained_source_len = items.len().saturating_sub(compaction_input_items);
    let group_starts = std::iter::once(0)
        .chain(
            items[..retained_source_len]
                .iter()
                .enumerate()
                .skip(1)
                .filter_map(|(index, item)| item.starts_turn.then_some(index)),
        )
        .collect::<Vec<_>>();
    let estimated_tokens_before = base_tokens.saturating_add(estimated_items_tokens(items));
    let target_tokens = budget.reduced_target(estimated_tokens_before);

    let protected_group_index = group_starts.len() - 1;
    let mut estimated_tokens_after = estimated_tokens_before;
    let mut removed = vec![false; items.len()];
    let mut removed_groups = 0;
    for group_index in 0..protected_group_index {
        if estimated_tokens_after <= target_tokens {
            break;
        }
        let group_start = group_starts[group_index];
        let group_end = group_starts[group_index + 1];
        estimated_tokens_after = estimated_tokens_after
            .saturating_sub(estimated_items_tokens(&items[group_start..group_end]));
        removed[group_start..group_end].fill(true);
        removed_groups += 1;
    }

    if estimated_tokens_after > target_tokens {
        let latest_group_start = group_starts[protected_group_index];
        for exchange in removable_tool_exchanges(
            items,
            latest_group_start..retained_source_len,
        ) {
            if estimated_tokens_after <= target_tokens {
                break;
            }
            for index in exchange {
                if !removed[index] {
                    estimated_tokens_after =
                        estimated_tokens_after.saturating_sub(items[index].estimated_tokens);
                    removed[index] = true;
                }
            }
        }
    }

    let removed_items = removed.iter().filter(|removed| **removed).count();
    if removed_items == 0 {
        return Err(CompactionReductionExhausted);
    }

    Ok(CompactionReduction {
        retained_indices: removed
            .into_iter()
            .enumerate()
            .filter_map(|(index, removed)| (!removed).then_some(index))
            .collect(),
        reduction: LocalCompactionReduction {
            removed_groups,
            removed_items,
            estimated_tokens_before,
            estimated_tokens_after,
            target_tokens,
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ToolExchangeKind {
    Function,
    Custom,
    Search,
}

fn tool_call_key(item: &ResponseItem) -> Option<(ToolExchangeKind, &str)> {
    match item {
        ResponseItem::FunctionCall { call_id, .. } => {
            Some((ToolExchangeKind::Function, call_id.as_str()))
        }
        ResponseItem::LocalShellCall {
            call_id: Some(call_id),
            ..
        } => Some((ToolExchangeKind::Function, call_id.as_str())),
        ResponseItem::CustomToolCall { call_id, .. } => {
            Some((ToolExchangeKind::Custom, call_id.as_str()))
        }
        ResponseItem::ToolSearchCall {
            call_id: Some(call_id),
            ..
        } => Some((ToolExchangeKind::Search, call_id.as_str())),
        ResponseItem::AdditionalTools { .. }
        | ResponseItem::Message { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { call_id: None, .. }
        | ResponseItem::ToolSearchCall { call_id: None, .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::ConfigurationUpdate { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::Other => None,
    }
}

fn tool_output_key(item: &ResponseItem) -> Option<(ToolExchangeKind, &str)> {
    match item {
        ResponseItem::FunctionCallOutput {
            call_id: Some(call_id),
            ..
        } => Some((ToolExchangeKind::Function, call_id.as_str())),
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            Some((ToolExchangeKind::Custom, call_id.as_str()))
        }
        ResponseItem::ToolSearchOutput {
            call_id: Some(call_id),
            ..
        } => Some((ToolExchangeKind::Search, call_id.as_str())),
        ResponseItem::AdditionalTools { .. }
        | ResponseItem::Message { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { call_id: None, .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::ToolSearchOutput { call_id: None, .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::ConfigurationUpdate { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::Other => None,
    }
}

fn removable_tool_exchanges(
    items: &[CompactionItem<'_>],
    range: std::ops::Range<usize>,
) -> Vec<Vec<usize>> {
    let mut claimed_outputs = std::collections::HashSet::new();
    let mut exchanges = Vec::new();
    for call_index in range.clone() {
        let Some(call_key) = tool_call_key(items[call_index].item) else {
            continue;
        };
        let Some(output_index) = range
            .clone()
            .find(|index| tool_output_key(items[*index].item) == Some(call_key))
        else {
            continue;
        };
        claimed_outputs.insert(output_index);
        exchanges.push(vec![call_index, output_index]);
    }
    for index in range {
        if claimed_outputs.contains(&index) {
            continue;
        }
        if matches!(
            items[index].item,
            ResponseItem::FunctionCallOutput { call_id: None, .. }
                | ResponseItem::ToolSearchOutput { call_id: None, .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::ImageGenerationCall { .. }
        ) {
            exchanges.push(vec![index]);
        }
    }
    exchanges.sort_by_key(|exchange| exchange[0]);
    exchanges
}

fn estimated_items_tokens(items: &[CompactionItem<'_>]) -> i64 {
    items
        .iter()
        .map(|item| item.estimated_tokens)
        .fold(0i64, i64::saturating_add)
}

#[cfg(test)]
#[path = "compaction_reduction_tests.rs"]
mod tests;
