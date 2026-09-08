//! Terminal-only correction for authoritative assistant output.
//!
//! Inline mode cannot rewrite rows that the terminal already owns. When the completed assistant
//! item differs from streamed source, the app appends this explicit correction after consolidating
//! canonical transcript state in memory. The cell is never added to `transcript_cells`.

use super::AgentMarkdownCell;
use super::HistoryCell;
use crate::inline_visualization::InlineVisualizationContext;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::visible_lines;
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use std::path::Path;

const CORRECTION_HEADING: &str = "Final response (corrected)";

#[derive(Debug)]
pub(crate) struct InlineCanonicalCorrectionCell {
    body: AgentMarkdownCell,
}

impl InlineCanonicalCorrectionCell {
    pub(crate) fn new(
        authoritative_source: String,
        cwd: &Path,
        inline_visualization_context: Option<InlineVisualizationContext>,
    ) -> Self {
        Self {
            body: AgentMarkdownCell::new_with_inline_visualizations(
                authoritative_source,
                cwd,
                inline_visualization_context,
            ),
        }
    }
}

impl HistoryCell for InlineCanonicalCorrectionCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        let mut lines = vec![HyperlinkLine::new(Line::from(CORRECTION_HEADING.bold()))];
        lines.extend(self.body.display_hyperlink_lines(width));
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::from(CORRECTION_HEADING)];
        lines.extend(self.body.raw_lines());
        lines
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }
}

#[cfg(test)]
#[path = "inline_correction_tests.rs"]
mod tests;
