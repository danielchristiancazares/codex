//! Case-folded search matches mapped back to original UTF-8 byte ranges.

use std::ops::Range;

pub(super) fn case_insensitive_match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }

    let query_lower = query
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if query_lower.is_empty() {
        return Vec::new();
    }

    let mut folded = String::new();
    let mut folded_spans: Vec<(Range<usize>, Range<usize>)> = Vec::new();
    for (original_start, ch) in text.char_indices() {
        let original_range = original_start..original_start + ch.len_utf8();
        for lower in ch.to_lowercase() {
            let folded_start = folded.len();
            folded.push(lower);
            folded_spans.push((folded_start..folded.len(), original_range.clone()));
        }
    }

    let mut ranges = Vec::new();
    let mut search_from = 0;
    while search_from <= folded.len()
        && let Some(relative_start) = folded[search_from..].find(&query_lower)
    {
        let folded_start = search_from + relative_start;
        let folded_end = folded_start + query_lower.len();
        if let Some((_, first_original)) = folded_spans.iter().find(|(folded_range, _)| {
            folded_range.end > folded_start && folded_range.start < folded_end
        }) {
            let original_end = folded_spans
                .iter()
                .rev()
                .find(|(folded_range, _)| {
                    folded_range.end > folded_start && folded_range.start < folded_end
                })
                .map(|(_, original_range)| original_range.end)
                .unwrap_or(first_original.end);
            ranges.push(first_original.start..original_end);
        }
        search_from = folded_end;
    }
    ranges
}

#[cfg(test)]
#[path = "history_match_ranges_tests.rs"]
mod tests;
