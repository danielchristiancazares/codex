//! Choose terminal-safe strategies for resizing the viewport and inserting history.

use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
use crate::insert_history::ResetScrollRegion;
use crate::insert_history::SetScrollRegion;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use ratatui::backend::Backend;
use ratatui::layout::Position;
use ratatui::layout::Size;
use std::io;
use std::io::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScrollbackStrategy {
    Standard,
    Zellij,
    FullScreen,
}

/// Discards up to `max_rows` of a tracked docking gap and moves the inline viewport with it.
///
/// Callers clear the live viewport first so deleting lines from this isolated region only moves
/// history and blank rows. The explicit delete-line sequence also discards a gap that starts at
/// row zero instead of copying it into terminal scrollback.
pub(crate) fn discard_docked_history_gap<B>(
    terminal: &mut Terminal<B>,
    screen_size: Size,
    max_rows: u16,
) -> io::Result<u16>
where
    B: Backend<Error = io::Error> + Write,
{
    let docked_gap_rows = terminal.docked_history_gap_rows();
    let discarded_rows = docked_gap_rows.min(max_rows);
    if discarded_rows == 0 {
        return Ok(0);
    }

    let gap_top = terminal
        .viewport_area
        .top()
        .saturating_sub(terminal.visible_history_rows())
        .saturating_sub(docked_gap_rows);
    let writer = terminal.backend_mut();
    queue!(
        writer,
        SetScrollRegion(gap_top.saturating_add(1)..screen_size.height),
        MoveTo(/*x*/ 0, gap_top),
        Print(format!("\x1b[{discarded_rows}M")),
        ResetScrollRegion,
    )?;
    terminal.consume_docked_history_gap(discarded_rows);
    let mut area = terminal.viewport_area;
    area.y = area.y.saturating_sub(discarded_rows);
    terminal.set_viewport_area(area);
    Ok(discarded_rows)
}

impl ScrollbackStrategy {
    pub(super) fn detect(terminal: &TerminalInfo) -> Self {
        if terminal.is_zellij() {
            Self::Zellij
        } else if terminal.name == TerminalName::WindowsTerminal
            || std::env::var_os("WT_SESSION").is_some()
        {
            Self::FullScreen
        } else {
            Self::Standard
        }
    }

    pub(super) fn history_insertion_mode(
        self,
        wrap_policy: HistoryLineWrapPolicy,
    ) -> InsertHistoryMode {
        match self {
            Self::FullScreen => InsertHistoryMode::FullScreen,
            Self::Zellij if wrap_policy == HistoryLineWrapPolicy::Terminal => {
                InsertHistoryMode::FullScreen
            }
            Self::Standard | Self::Zellij => InsertHistoryMode::Standard,
        }
    }

    pub(super) fn grow_viewport<B>(
        self,
        terminal: &mut Terminal<B>,
        viewport_top: u16,
        screen_size: Size,
        scroll_by: u16,
    ) -> io::Result<()>
    where
        B: Backend<Error = io::Error> + Write,
    {
        match self {
            Self::FullScreen => {
                // Partial DEC scroll regions can discard rows instead of moving them into Windows
                // Terminal's scrollback. Clear the stale composer, discard any tracked docking
                // gap, then scroll the remaining rows across the entire screen.
                terminal.clear_after_position(Position::new(/*x*/ 0, viewport_top))?;
                let discarded_rows = discard_docked_history_gap(terminal, screen_size, scroll_by)?;
                let scroll_by = scroll_by.saturating_sub(discarded_rows);
                let writer = terminal.backend_mut();
                queue!(
                    writer,
                    MoveTo(/*x*/ 0, screen_size.height.saturating_sub(/*rhs*/ 1))
                )?;
                for _ in 0..scroll_by {
                    queue!(writer, Print("\r\n"))?;
                }
                Ok(())
            }
            Self::Standard | Self::Zellij => terminal
                .backend_mut()
                .scroll_region_up(0..viewport_top, scroll_by),
        }
    }

    /// Moves a sparse, contiguous history tail with a bottom-docked viewport as it shrinks.
    ///
    /// Returns whether the physical terminal rows were moved. The caller skips this operation
    /// while the terminal itself is resizing because terminal reflow owns those physical rows.
    pub(super) fn dock_sparse_history_tail<B>(
        self,
        terminal: &mut Terminal<B>,
        previous_viewport_top: u16,
        viewport_top: u16,
    ) -> io::Result<bool>
    where
        B: Backend<Error = io::Error> + Write,
    {
        if viewport_top <= previous_viewport_top {
            return Ok(false);
        }

        let history_rows = terminal.visible_history_rows();
        if history_rows == 0 {
            return Ok(false);
        }
        let scroll_by = viewport_top - previous_viewport_top;
        let history_top = previous_viewport_top - history_rows;
        // Shrinking the viewport moves retained rows toward the bottom of the screen; no row needs
        // to cross the screen edge into terminal scrollback. Limit the operation to the tracked
        // history tail and the vacated viewport rows so the full-screen Windows strategy leaves
        // unrelated shell output in place. Track its vacated band for the next history insertion.
        let region = history_top..viewport_top;
        terminal
            .backend_mut()
            .scroll_region_down(region, scroll_by)?;
        match self {
            Self::FullScreen => terminal.note_docked_history_gap(scroll_by),
            Self::Standard | Self::Zellij => {}
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "scrollback_tests.rs"]
mod tests;
