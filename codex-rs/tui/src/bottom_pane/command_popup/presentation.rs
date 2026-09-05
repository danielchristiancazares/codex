//! The selected command gets its own readable description. Narrow menus use one column;
//! wider menus retain brief descriptions for scanning. Detail space is stable during navigation.

use super::*;
use crate::bottom_pane::selection_popup_common::ColumnWidthConfig;
use crate::bottom_pane::selection_popup_common::ColumnWidthMode;
use crate::bottom_pane::selection_popup_common::render_rows_single_line_with_col_width_mode;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::style::secondary_style;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

const COMPACT_WIDTH: u16 = 60;
const COMMAND_COLUMN_WIDTH: ColumnWidthConfig = ColumnWidthConfig::new(
    ColumnWidthMode::AutoAllRows,
    /*name_column_width*/ None,
);

impl CommandPopup {
    fn description_rows(&self, width: u16) -> u16 {
        if self.selected_item().is_none() {
            0
        } else if width < COMPACT_WIDTH {
            4
        } else {
            2
        }
    }

    /// Reserve a stable detail area so arrow-key navigation keeps the composer stationary.
    pub(crate) fn calculate_required_height(&self, width: u16) -> u16 {
        let items = u16::try_from(self.filtered_items().len().clamp(1, MAX_POPUP_ROWS)).unwrap_or(u16::MAX);
        let description = self.description_rows(width);
        items.saturating_add(description).saturating_add(u16::from(description > 0))
    }
}

impl WidgetRef for CommandPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut rows = self.rows_from_matches(self.filtered());
        if area.width < COMPACT_WIDTH {
            for row in &mut rows {
                row.description = None;
            }
        }
        let description_height = self.description_rows(area.width).min(area.height.saturating_sub(2));
        let list_height = area.height.saturating_sub(description_height).saturating_sub(u16::from(description_height > 0));
        let list_area = Rect::new(area.x, area.y, area.width, list_height);
        render_rows_single_line_with_col_width_mode(
            list_area,
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            "No matching commands",
            COMMAND_COLUMN_WIDTH,
        );
        if description_height == 0 {
            return;
        }
        let Some(selected) = self.selected_item() else { return; };
        let options = textwrap::Options::new(usize::from(area.width)).initial_indent("  ").subsequent_indent("  ");
        let wrapped = textwrap::wrap(selected.description(), options);
        for (row, text) in wrapped.iter().take(usize::from(description_height)).enumerate() {
            let text = if row + 1 == usize::from(description_height) && wrapped.len() > usize::from(description_height) {
                format!("{text}…")
            } else {
                text.to_string()
            };
            truncate_line_with_ellipsis_if_overflow(Line::from(Span::styled(text, secondary_style())), usize::from(area.width))
                .render(Rect::new(area.x, list_area.bottom() + 1 + row as u16, area.width, /*height*/ 1), buf);
        }
    }
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
