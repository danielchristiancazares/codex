//! Refill newly available visible history without clearing terminal scrollback.

use std::collections::VecDeque;

use color_eyre::eyre::Result;
use ratatui::layout::Size;
use ratatui::text::Line;

use super::App;
use crate::insert_history::wrap_history_hyperlink_lines;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::transcript_reflow::TranscriptReplayPolicy;
use crate::tui::Tui;

impl App {
    pub(super) fn refill_history_after_viewport_shrink(
        &self,
        tui: &mut Tui,
        screen_size: Size,
        viewport_height: u16,
    ) -> Result<()> {
        let viewport_height = viewport_height.min(screen_size.height);
        if self.transcript_replay_policy != TranscriptReplayPolicy::OwnedBufferReplay
            || self.chat_widget.has_active_agent_stream()
            || self.chat_widget.has_active_plan_stream()
            || self.chat_widget.raw_output_mode()
            || tui.terminal.viewport_area.width != screen_size.width
            || viewport_height >= tui.terminal.viewport_area.height
        {
            return Ok(());
        }
        let row_budget = usize::from(screen_size.height.saturating_sub(viewport_height));
        let width = self.chat_widget.history_wrap_width(screen_size.width);
        let wrap_policy = self.history_line_wrap_policy();
        let mut retained = VecDeque::with_capacity(row_budget);
        let mut separator_before_tail = false;
        'cells: for cell in self.transcript_cells.iter().rev() {
            if retained.len() == row_budget {
                break;
            }
            let lines = cell
                .display_hyperlink_lines_for_mode(width, self.chat_widget.history_render_mode());
            if lines.is_empty() {
                continue;
            }
            let (lines, rows) = wrap_history_hyperlink_lines(
                &lines,
                usize::from(screen_size.width.max(/*other*/ 1)),
                wrap_policy,
            );
            // Native-wrapped URLs and raw output retain their terminal-owned physical rows.
            if rows != lines.len() {
                return Ok(());
            }
            if separator_before_tail {
                retained.push_front(HyperlinkLine::new(Line::default()));
            }
            for line in lines.into_iter().rev() {
                if retained.len() == row_budget {
                    break 'cells;
                }
                retained.push_front(line);
            }
            separator_before_tail = !cell.is_stream_continuation();
        }
        tui.refill_visible_history(viewport_height, retained.make_contiguous(), wrap_policy)?;
        Ok(())
    }
}
