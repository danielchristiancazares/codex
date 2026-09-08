//! A quiet, bounded starting surface. The live composer retains ownership of input and its cursor.
//! The app shows this only while the transcript contains session metadata; the first real history
//! item returns to the normal bottom-docked conversation and its existing scrollback behavior.

use super::*;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::start_screen::StartScreen;
use crate::start_screen::StartScreenIdentity;
use crate::start_screen::StartScreenPresentation;
use crate::style::accent_style;
use crate::style::key_hint_style;
use crate::style::secondary_style;
use ratatui::text::Span;

impl ChatWidget {
    pub(crate) fn landing_surface(&self) -> Option<StartScreen<'_>> {
        let phase = match (self.thread_id, self.transcript.active_cell.as_ref()) {
            (None, Some(cell)) if cell.as_any().is::<history_cell::SessionHeaderHistoryCell>() => {
                LandingPhase::Preparing
            }
            (Some(_), None) => LandingPhase::Ready,
            _ => return None,
        };
        if self.blocks_direct_input && phase == LandingPhase::Ready
            || self.active_side_conversation
            || self
                .selected_index_for_present_view(crate::app::AGENTS_OVERVIEW_VIEW_ID)
                .is_some()
            || self.is_user_turn_pending_or_running()
            || self.bottom_pane.is_task_running()
            || self.last_rendered_user_message_display.is_some()
            || !self.input_queue.queued_user_messages.is_empty()
            || !self.input_queue.pending_steers.is_empty()
        {
            return None;
        }
        Some(StartScreen {
            presentation: if self.has_active_view() {
                StartScreenPresentation::Selection
            } else if !self.bottom_pane.no_modal_or_popup_active() {
                StartScreenPresentation::Completion
            } else {
                StartScreenPresentation::Introduction
            },
            composer: self
                .bottom_pane
                .as_renderable_with_composer_right_reserve(/*composer_right_reserve*/ 0),
            header: RenderableItem::Owned(Box::new(LandingHeader {
                phase,
                permissions: if history_cell::is_yolo_mode(&self.config) {
                    "Unrestricted access"
                } else {
                    "Guarded access"
                },
            })),
            actions: if phase == LandingPhase::Ready && self.bottom_pane.no_modal_or_popup_active()
            {
                RenderableItem::Owned(Box::new(LandingActions))
            } else {
                RenderableItem::Owned(Box::new(()))
            },
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LandingPhase {
    Preparing,
    Ready,
}

struct LandingHeader {
    phase: LandingPhase,
    permissions: &'static str,
}

impl Renderable for LandingHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let wordmark = self.phase == LandingPhase::Ready && area.width >= 40 && area.height >= 9;
        let title = Line::from(vec![
            Span::styled(">_ ", accent_style()),
            "Codex".bold(),
            if self.phase == LandingPhase::Preparing && area.height == 1 {
                Span::styled(" · loading", secondary_style())
            } else {
                "".into()
            },
        ]);
        let (subtitle, description) = match self.phase {
            LandingPhase::Preparing => (
                "Getting ready…",
                "Draft now. Queued messages send when ready.",
            ),
            LandingPhase::Ready => (
                "Make something worth keeping.",
                if area.width < 46 {
                    "Ask a question. Shape an idea. Create."
                } else {
                    "Ask a question. Shape an idea. Make a change."
                },
            ),
        };
        let lines = [
            if wordmark { Line::default() } else { title },
            Line::default(),
            Line::from(subtitle.bold()),
            Line::from(Span::styled(description, secondary_style())),
            Line::default(),
            Line::from(vec![
                self.permissions.into(),
                if self.phase == LandingPhase::Ready {
                    " · ".dim()
                } else {
                    "".into()
                },
                if self.phase == LandingPhase::Ready {
                    Span::styled("/permissions", key_hint_style()).underlined()
                } else {
                    "".into()
                },
            ]),
        ];
        if wordmark {
            StartScreenIdentity.render(Rect::new(area.x, area.y, area.width, /*height*/ 3), buf);
        }
        for (row, line) in lines.into_iter().enumerate() {
            let row = if wordmark && row > 0 { row + 2 } else { row };
            if row < usize::from(area.height) {
                truncate_line_with_ellipsis_if_overflow(line, usize::from(area.width)).render(
                    Rect::new(area.x, area.y + row as u16, area.width, /*height*/ 1),
                    buf,
                );
            }
        }
    }

    fn desired_height(&self, _width: u16) -> u16 {
        8
    }
}

struct LandingActions;

impl Renderable for LandingActions {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let actions_width = area.width;
        let action_row = Line::from(vec![
            Span::styled("/plan", key_hint_style()).underlined(),
            Span::styled("  Shape an idea     ", secondary_style()),
            Span::styled("/review", key_hint_style()).underlined(),
            Span::styled("  Review changes     ", secondary_style()),
            Span::styled("/resume", key_hint_style()).underlined(),
            Span::styled("  Pick up a thread", secondary_style()),
        ]);
        let actions = if action_row.width() <= usize::from(actions_width) {
            vec![action_row]
        } else {
            [
                ("/plan", "Shape an idea"),
                ("/review", "Review changes"),
                ("/resume", "Pick up a thread"),
            ]
            .into_iter()
            .map(|(command, label)| {
                Line::from(vec![
                    Span::styled(format!("{command:<9}"), key_hint_style()),
                    Span::styled(label, secondary_style()),
                ])
            })
            .collect()
        };
        for (row, line) in actions.into_iter().enumerate() {
            if row < usize::from(area.height) {
                truncate_line_with_ellipsis_if_overflow(line, usize::from(actions_width)).render(
                    Rect::new(
                        area.x,
                        area.y + row as u16,
                        actions_width,
                        /*height*/ 1,
                    ),
                    buf,
                );
            }
        }
    }

    fn desired_height(&self, _width: u16) -> u16 {
        3
    }
}

#[cfg(test)]
#[path = "landing_tests.rs"]
mod tests;
