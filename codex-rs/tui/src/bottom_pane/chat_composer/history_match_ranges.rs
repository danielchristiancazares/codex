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
    // Each pointer advances monotonically through the folded UTF-8 spans.
    let mut start_span = 0;
    let mut end_span = 0;
    while search_from <= folded.len()
        && let Some(relative_start) = folded[search_from..].find(&query_lower)
    {
        let folded_start = search_from + relative_start;
        let folded_end = folded_start + query_lower.len();
        while folded_spans[start_span].0.end <= folded_start {
            start_span += 1;
        }
        while folded_spans[end_span].0.end < folded_end {
            end_span += 1;
        }
        ranges.push(folded_spans[start_span].1.start..folded_spans[end_span].1.end);
        search_from = folded_end;
    }
    ranges
}

#[cfg(test)]
#[path = "history_match_ranges_tests.rs"]
mod tests;
