//! Assistant turn markers defer to a rendered leading Markdown heading. The two-column gutter
//! remains stable across streaming chunks, finalized messages, and transcript reflow.

use ratatui::style::Modifier;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

pub(super) fn assistant_turn_prefix(first_line: &Line<'_>) -> Span<'static> {
    let starts_with_heading = first_line.spans.first().is_some_and(|span| {
        span.style.sub_modifier.contains(Modifier::DIM)
            && span.content.strip_suffix(' ').is_some_and(|marker| {
                (1..=6).contains(&marker.len()) && marker.bytes().all(|byte| byte == b'#')
            })
    });
    if starts_with_heading {
        "  ".into()
    } else {
        "• ".dim()
    }
}
