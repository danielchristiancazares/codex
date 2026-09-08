//! Compact status metadata without losing the identifying end of an absolute workspace path.
//!
//! Status items arrive as separate spans. Paths prefer at most a third of the row and shed their
//! parent directories before complete model/reasoning labels. A leaf can stand on its own.
//! If the remaining items still overflow, the longest spans share a width cap so
//! short trailing states remain visible. Configured order and styles stay intact.

use ratatui::text::Line;
use unicode_segmentation::UnicodeSegmentation;

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::width::display_width;

pub(super) fn truncate_status_line(mut line: Line<'static>, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return truncate_line_with_ellipsis_if_overflow(line, max_width);
    }
    let mut overflow = line.width().saturating_sub(max_width);
    let mut path_indices = Vec::new();
    for (index, span) in line.spans.iter_mut().enumerate() {
        let text = span.content.as_ref();
        let is_path = text.starts_with('/')
            || text.starts_with('\\')
            || text.starts_with("~/")
            || text.starts_with("~\\")
            || (text.as_bytes().get(1) == Some(&b':')
                && matches!(text.as_bytes().get(2), Some(b'/' | b'\\')));
        if !is_path {
            continue;
        }
        path_indices.push(index);
        let previous_width = span.width();
        let leaf_width = text
            .rsplit(['/', '\\'])
            .next()
            .map(display_width)
            .unwrap_or(0);
        let available = previous_width
            .saturating_sub(overflow)
            .min((max_width / 3).min(/*other*/ 32))
            .max(leaf_width.min(previous_width));
        span.content = compact_path(text, available).into();
        overflow = overflow.saturating_sub(previous_width.saturating_sub(span.width()));
    }
    if overflow == 0 {
        return line;
    }
    // Preserve complete short states (for example, title-generation progress)
    // before trimming a second item at the right edge of the footer.
    let mut lower = 0;
    let mut upper = line
        .spans
        .iter()
        .map(ratatui::prelude::Span::width)
        .max()
        .unwrap_or(0);
    while lower < upper {
        let candidate = lower + (upper - lower).div_ceil(2);
        let width: usize = line
            .spans
            .iter()
            .map(|span| span.width().min(candidate))
            .sum();
        if width <= max_width {
            lower = candidate;
        } else {
            upper = candidate - 1;
        }
    }
    let reserved: usize = line.spans.iter().map(|span| span.width().min(lower)).sum();
    let mut remaining = max_width.saturating_sub(reserved);
    for (index, span) in line.spans.iter_mut().enumerate() {
        let extra = span.width().saturating_sub(lower).min(remaining);
        remaining -= extra;
        let width = lower + extra;
        span.content = if path_indices.contains(&index) {
            compact_path(span.content.as_ref(), width)
        } else {
            let truncated =
                truncate_line_with_ellipsis_if_overflow(Line::from(span.clone()), width)
                    .to_string();
            if span.width() > width
                && let Some((prefix, _)) = truncated.trim_end_matches('…').rsplit_once(' ')
                && display_width(prefix) > width / 2
            {
                format!("{}…", prefix.trim_end_matches([' ', '·']))
            } else {
                truncated
            }
        }
        .into();
    }
    line
}

fn compact_path(text: &str, available: usize) -> String {
    if available == 0 {
        return String::new();
    }
    if display_width(text) <= available {
        return text.to_string();
    }
    let leaf = text.rsplit(['/', '\\']).next().unwrap_or(text);
    if display_width(leaf) <= available && display_width(leaf).saturating_add(2) > available {
        return leaf.to_string();
    }
    text.match_indices(['/', '\\'])
        .map(|(index, _)| &text[index..])
        .find(|suffix| display_width(suffix).saturating_add(1) <= available)
        .map(|suffix| format!("…{suffix}"))
        .unwrap_or_else(|| {
            let mut used = 1;
            let mut suffix = Vec::new();
            for grapheme in text.graphemes(/*is_extended*/ true).rev() {
                used += display_width(grapheme);
                if used > available {
                    break;
                }
                suffix.push(grapheme);
            }
            format!("…{}", suffix.into_iter().rev().collect::<String>())
        })
}

#[cfg(test)]
#[path = "status_line_layout_tests.rs"]
mod tests;
