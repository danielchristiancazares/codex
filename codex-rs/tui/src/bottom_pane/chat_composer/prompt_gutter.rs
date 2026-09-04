//! Resolves which composer row owns the prompt caret for one frame.
//!
//! The composer reserves a single column to the left of its text area for a caret glyph. That
//! caret is the composer's primary focus cue, so exactly one row may own it at a time:
//!
//! - Normally the caret sits on the first text-area row, styled by the active reasoning tier, or
//!   by the shell accent while a `!` command is being composed.
//! - While a remote `[Image #N]` row is selected, keys are routed to that row instead of the text
//!   area and the terminal cursor is hidden. The caret moves up to the selected row so the focus
//!   change is legible without relying on color or reverse video.
//! - When input is disabled no row accepts keys, so the caret stays dim on the text-area row.
//!
//! Only the caret's position and style change; the column itself is always reserved, so composer
//! geometry stays stable across all three states.

use ratatui::style::Stylize;
use ratatui::text::Span;

use super::super::effort_ignition::EffortTier;
use crate::style::accent_style;

/// Composer state that decides where the prompt caret is drawn.
#[derive(Clone, Copy, Debug)]
pub(super) struct PromptGutterState {
    pub(super) input_enabled: bool,
    pub(super) is_bash_mode: bool,
    pub(super) effort_tier: Option<EffortTier>,
    /// Ignition charge in `0.0..=1.0` used to fade in the reasoning-tier caret.
    pub(super) ignition_charge: f32,
    /// Index of the remote image row that currently owns keyboard focus.
    pub(super) selected_remote_image_index: Option<usize>,
}

/// The caret placement chosen for one frame.
#[derive(Clone, Debug)]
pub(super) struct PromptGutter {
    /// Caret drawn on the first text-area row, if that row should show one.
    pub(super) textarea: Option<Span<'static>>,
    /// Remote image row index and caret, when a remote row owns focus.
    pub(super) remote_image_row: Option<(usize, Span<'static>)>,
}

impl PromptGutterState {
    pub(super) fn resolve(self) -> PromptGutter {
        if !self.input_enabled {
            return PromptGutter {
                textarea: Some("›".dim()),
                remote_image_row: None,
            };
        }

        let caret = if self.is_bash_mode {
            Span::from("!").light_red().bold()
        } else if let Some(tier) = self.effort_tier {
            tier.prompt(self.ignition_charge)
        } else {
            Span::styled("›", accent_style())
        };

        match self.selected_remote_image_index {
            Some(index) => PromptGutter {
                textarea: None,
                remote_image_row: Some((index, caret)),
            },
            None => PromptGutter {
                textarea: Some(caret),
                remote_image_row: None,
            },
        }
    }
}

#[cfg(test)]
#[path = "prompt_gutter_tests.rs"]
mod tests;
