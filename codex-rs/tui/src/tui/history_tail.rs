//! Update visible terminal-history tails without discarding terminal scrollback.

use super::Tui;
use crate::custom_terminal::Terminal as CustomTerminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
use crate::insert_history::insert_history_hyperlink_lines_with_mode_and_wrap_policy;
use crate::insert_history::wrap_history_hyperlink_lines;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::backend::Backend;
use ratatui::layout::Position;
use std::io;
use std::io::Write;

impl Tui {
    #[cfg(test)]
    pub(crate) fn pending_history_lines_for_test(&self) -> Vec<HyperlinkLine> {
        self.pending_history_lines
            .iter()
            .flat_map(|batch| batch.lines.iter().cloned())
            .collect()
    }

    pub(crate) fn replace_visible_history_tail(
        &mut self,
        previous_lines: &[HyperlinkLine],
        replacement: &[HyperlinkLine],
        wrap_policy: HistoryLineWrapPolicy,
    ) -> io::Result<bool> {
        let screen_size = self.terminal.last_known_screen_size;
        Self::flush_pending_history_lines(
            &mut self.terminal,
            &mut self.pending_history_lines,
            self.scrollback,
            screen_size,
        )?;
        let mode = self.scrollback.history_insertion_mode(wrap_policy);
        let replaced = replace_visible_terminal_history_tail(
            &mut self.terminal,
            previous_lines,
            replacement,
            mode,
            wrap_policy,
        )?;
        if replaced {
            self.frame_requester().schedule_frame();
        }
        Ok(replaced)
    }

    /// Refill the Codex-owned visible history band when the live viewport gets shorter.
    ///
    /// The caller supplies rendered source rows for this width. Replacement stays inside the
    /// previously tracked history band plus the released viewport rows, leaving older terminal
    /// scrollback and unrelated shell output untouched.
    pub(crate) fn refill_visible_history(
        &mut self,
        viewport_height: u16,
        replacement: &[HyperlinkLine],
        wrap_policy: HistoryLineWrapPolicy,
    ) -> io::Result<()> {
        // Initial/rebuilt history is still queued under the former full-height surface.
        // Let the next draw shorten that surface before inserting its rows into the terminal.
        if self.terminal.visible_history_rows() == 0
            || viewport_height >= self.terminal.viewport_area.height
        {
            return Ok(());
        }
        let screen_size = self.terminal.last_known_screen_size;
        Self::flush_pending_history_lines(
            &mut self.terminal,
            &mut self.pending_history_lines,
            self.scrollback,
            screen_size,
        )?;
        let previous = self.terminal.viewport_area;
        let previous_rows = self.terminal.visible_history_rows();
        let released_rows = previous.height.saturating_sub(viewport_height);
        let row_budget = usize::from(previous_rows.saturating_add(released_rows));
        let replacement = &replacement[replacement.len().saturating_sub(row_budget)..];
        if previous_rows == 0
            || released_rows == 0
            || replacement.len() <= usize::from(previous_rows)
        {
            return Ok(());
        }
        self.terminal.set_viewport_area(ratatui::layout::Rect {
            height: viewport_height,
            ..previous
        });
        replace_visible_terminal_history_rows(
            &mut self.terminal,
            previous_rows,
            replacement,
            self.scrollback.history_insertion_mode(wrap_policy),
            wrap_policy,
        )?;
        Ok(())
    }
}

fn replace_visible_terminal_history_tail<B>(
    terminal: &mut CustomTerminal<B>,
    previous_lines: &[HyperlinkLine],
    replacement: &[HyperlinkLine],
    mode: InsertHistoryMode,
    wrap_policy: HistoryLineWrapPolicy,
) -> io::Result<bool>
where
    B: Backend<Error = io::Error> + Write,
{
    let viewport = terminal.viewport_area;
    let wrap_width = usize::from(viewport.width.max(/*other*/ 1));
    let (_, previous_rows) = wrap_history_hyperlink_lines(previous_lines, wrap_width, wrap_policy);
    let Ok(previous_rows) = u16::try_from(previous_rows) else {
        return Ok(false);
    };
    replace_visible_terminal_history_rows(terminal, previous_rows, replacement, mode, wrap_policy)
}

fn replace_visible_terminal_history_rows<B>(
    terminal: &mut CustomTerminal<B>,
    previous_rows: u16,
    replacement: &[HyperlinkLine],
    mode: InsertHistoryMode,
    wrap_policy: HistoryLineWrapPolicy,
) -> io::Result<bool>
where
    B: Backend<Error = io::Error> + Write,
{
    let screen_size = terminal.last_known_screen_size;
    let mut viewport = terminal.viewport_area;
    if previous_rows == 0 || previous_rows > viewport.top() {
        return Ok(false);
    }

    viewport.y -= previous_rows;
    terminal.clear_after_position(Position::new(/*x*/ 0, viewport.y))?;
    terminal.note_history_rows_removed(previous_rows);
    terminal.set_viewport_area(viewport);
    terminal.invalidate_viewport();
    insert_history_hyperlink_lines_with_mode_and_wrap_policy(
        terminal,
        replacement,
        mode,
        wrap_policy,
        screen_size,
    )?;
    Ok(true)
}

#[cfg(test)]
#[path = "history_tail_tests.rs"]
mod tests;
