use super::*;
use crate::history_cell::HistoryCell;
use crate::terminal_hyperlinks::visible_lines;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn correction_cell_renders_heading_markdown_and_wrapping() {
    let cell = InlineCanonicalCorrectionCell::new(
        "Corrected **bold text** with a [local link](README.md:93) and enough words to wrap."
            .to_string(),
        Path::new("/workspace"),
        /*inline_visualization_context*/ None,
    );

    let rendered = visible_lines(cell.display_hyperlink_lines(/*width*/ 34));
    let plain = rendered
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        plain.first().map(String::as_str),
        Some("Final response (corrected)")
    );
    insta::assert_debug_snapshot!("inline_canonical_correction_markdown_wrapping", rendered);
}
