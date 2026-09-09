//! Keep tracked terminal history adjacent to every inline frame.
//!
//! Popup filtering and nested selections can change height several times before returning to the
//! composer. Each transition must move the entire tracked tail with its viewport: leaving a gap
//! below that tail makes the next transition mistake blank rows for history and split the transcript.

use std::io;
use std::io::Result;
use std::io::Write;

use ratatui::backend::Backend;
use ratatui::layout::Size;

use super::InlineViewportPlacement;
use super::InlineViewportRole;
use super::PendingHistoryLines;
use super::Tui;
use super::scrollback::HistoryTailDock;
use super::scrollback::ScrollbackStrategy;
use crate::custom_terminal::Terminal;

pub(super) struct InlineViewportFrame {
    pub(super) height: u16,
    pub(super) screen_size: Size,
    pub(super) placement: InlineViewportPlacement,
    pub(super) role: InlineViewportRole,
    pub(super) previous_role: InlineViewportRole,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ViewportRepaint {
    ReuseDiff,
    InvalidateDiff,
}

impl ViewportRepaint {
    fn apply<B>(self, terminal: &mut Terminal<B>)
    where
        B: Backend<Error = io::Error> + Write,
    {
        match self {
            Self::ReuseDiff => {}
            Self::InvalidateDiff => terminal.invalidate_viewport(),
        }
    }
}

impl InlineViewportFrame {
    pub(super) fn prepare<B>(
        self,
        terminal: &mut Terminal<B>,
        pending_history: &mut Vec<PendingHistoryLines>,
        scrollback: ScrollbackStrategy,
    ) -> Result<()>
    where
        B: Backend<Error = io::Error> + Write,
    {
        // Continuous output may use released live rows before docking. Popup transitions restore
        // the anchor first, so even a short queued batch ends directly above the composer.
        let history_tail_dock = if scrollback == ScrollbackStrategy::FullScreen
            && self.role == InlineViewportRole::Persistent
            && self.previous_role == InlineViewportRole::Persistent
            && !pending_history.is_empty()
        {
            HistoryTailDock::DeferToPendingHistory
        } else {
            HistoryTailDock::Immediate
        };
        let repaint = Tui::update_inline_viewport_for_resize_reflow(
            terminal,
            self.height,
            self.screen_size,
            self.placement,
            scrollback,
            history_tail_dock,
        )?;
        Tui::flush_pending_history_lines(terminal, pending_history, scrollback, self.screen_size)?;
        repaint.apply(terminal);
        Ok(())
    }
}

#[cfg(test)]
#[path = "inline_viewport_tests.rs"]
mod tests;
