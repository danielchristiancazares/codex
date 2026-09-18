//! Fits a bounded summary and retained user input using the caller's request projection.

use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_text;

use crate::RequestBudget;

#[derive(Debug, thiserror::Error)]
#[error("local compaction replacement cannot fit its required context in the model window")]
pub struct ReplacementExceedsWindow;

pub struct CompactionReplacement<T> {
    pub items: T,
    pub summary_text: String,
}

/// Each candidate must include required context and its complete request estimate.
/// A zero user-message allowance must omit all retained user messages.
pub fn fit_compaction_replacement<T>(
    summary_text: &str,
    max_user_message_tokens: usize,
    budget: RequestBudget,
    canonical_summary: impl Fn(&str) -> String,
    candidate: impl Fn(&str, usize) -> (T, i64),
) -> Result<CompactionReplacement<T>, ReplacementExceedsWindow> {
    // Seed each bounded search with a fitting concrete candidate.
    let summary_token_limit = approx_token_count(summary_text).min(10_000);
    let mut bounded_summary = canonical_summary("");
    let (_, estimated_tokens) = candidate(&bounded_summary, /*user_message_token_limit*/ 0);
    budget
        .check(estimated_tokens)
        .map_err(|_| ReplacementExceedsWindow)?;
    let mut lower = 0usize;
    let mut upper = summary_token_limit;
    while lower <= upper {
        let allowance = lower + (upper - lower) / 2;
        let summary = canonical_summary(&truncate_text(
            summary_text,
            TruncationPolicy::Tokens(allowance),
        ));
        let (_, estimated_tokens) = candidate(&summary, /*user_message_token_limit*/ 0);
        if approx_token_count(&summary) <= 10_000 && budget.check(estimated_tokens).is_ok() {
            bounded_summary = summary;
            lower = allowance.saturating_add(1);
        } else if allowance == 0 {
            break;
        } else {
            upper = allowance - 1;
        }
    }
    let (mut fitted_items, _) = candidate(&bounded_summary, /*user_message_token_limit*/ 0);
    let mut lower = 0usize;
    let mut upper = max_user_message_tokens;
    while lower <= upper {
        let allowance = lower + (upper - lower) / 2;
        let (items, estimated_tokens) = candidate(&bounded_summary, allowance);
        if budget.check(estimated_tokens).is_ok() {
            fitted_items = items;
            lower = allowance.saturating_add(1);
        } else if allowance == 0 {
            break;
        } else {
            upper = allowance - 1;
        }
    }
    Ok(CompactionReplacement {
        items: fitted_items,
        summary_text: bounded_summary,
    })
}
