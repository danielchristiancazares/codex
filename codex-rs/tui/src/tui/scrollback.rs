//! Choose terminal-safe strategies for resizing the viewport and inserting history.

use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
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
                // Terminal's scrollback. Clear the stale composer, then scroll the entire screen.
                terminal.clear_after_position(Position::new(/*x*/ 0, viewport_top))?;
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
        // history tail and the vacated viewport rows so the full-screen Windows strategy does not
        // shift unrelated shell output or introduce blank rows above it.
        let region = history_top..viewport_top;
        terminal
            .backend_mut()
            .scroll_region_down(region, scroll_by)?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "scrollback_tests.rs"]
mod tests;
