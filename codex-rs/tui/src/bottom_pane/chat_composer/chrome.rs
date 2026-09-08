//! Composer framing and contextual action cues. Painting never changes input semantics.
//! Neutral surfaces follow the terminal palette; low-color terminals retain a complete outline.

use super::*;
use crate::color::blend;
use crate::style::accent_style;
use crate::style::key_hint_style;
use crate::style::secondary_style;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::best_color_for_level;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::effective_stdout_color_level;
use ratatui::widgets::BorderType;

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod tests;

impl ChatComposer {
    pub(super) fn render_composer_frame(&self, area: Rect, buf: &mut Buffer) -> Style {
        let level = effective_stdout_color_level();
        let mut surface = user_message_style();
        if matches!(
            level,
            StdoutColorLevel::TrueColor | StdoutColorLevel::Ansi256
        ) && let (Some(fg), Some(bg)) = (default_fg(), default_bg())
        {
            surface = surface.bg(best_color_for_level(blend(fg, bg, /*alpha*/ 0.055), level));
        }
        if self.blocks_direct_input || !self.draft.input_enabled {
            surface = secondary_style().bg(Color::Reset);
        }
        let compact = self.config.compact_two_row_layout && area.height == 2;
        let borders = if compact {
            Borders::TOP | Borders::LEFT | Borders::RIGHT
        } else {
            Borders::ALL
        };
        let mut frame = Block::default()
            .borders(borders)
            .border_type(BorderType::Rounded)
            .border_style(table_separator_style());
        let normal_input = self.draft.input_enabled
            && !self.blocks_direct_input
            && self.config.slash_commands_enabled
            && self.history_search.is_none()
            && self.draft.textarea.vim_query().is_none()
            && matches!(
                self.footer_mode(),
                FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft
            );
        if area.width >= 16 && (self.blocks_direct_input || !self.draft.input_enabled) {
            let label = if self.blocks_direct_input {
                " Read-only "
            } else {
                " Unavailable "
            };
            frame = frame.title_top(Line::from(label).style(secondary_style()));
        }
        if normal_input && area.width >= 16 {
            let hint_style = Style::default().fg(Color::Reset).not_dim();
            let label = if area.width >= 26 && self.draft.is_bash_mode {
                " Shell "
            } else {
                ""
            };
            let title_style = if self.has_focus {
                accent_style()
            } else {
                key_hint_style()
            };
            if !label.is_empty() {
                frame = frame.title_top(Line::from(label).style(title_style.not_dim()));
            }
            let popup_selection = match &self.popups.active {
                ActivePopup::None => false,
                ActivePopup::Command(popup) => popup.selected_item().is_some(),
                ActivePopup::File(popup) => popup.selected_match().is_some(),
                ActivePopup::Skill(popup) => popup.selected_mention().is_some(),
                ActivePopup::MentionV2(popup) => popup.selected().is_some(),
            };
            // Popup selection currently owns plain Enter, independently of the submit keymap.
            let action_hint = if popup_selection {
                Some((key_hint::plain(KeyCode::Enter), " select "))
            } else if self.queue_submissions {
                self.submit_keys
                    .first()
                    .copied()
                    .map(|binding| (binding, " queue "))
            } else if self.is_task_running && !self.draft.is_bash_mode && !self.popup_active() {
                self.queue_keys
                    .first()
                    .copied()
                    .map(|binding| (binding, " queue "))
            } else {
                None
            };
            if let Some((binding, action)) = action_hint {
                let hint = Line::from(vec![
                    " ".into(),
                    binding.into(),
                    Span::styled(action, secondary_style()),
                ])
                .style(hint_style)
                .right_aligned();
                if hint.width() + label.len() + 6 <= usize::from(area.width) {
                    frame = frame.title_top(hint);
                }
            }
            if !compact && area.height >= 3 && area.width >= 40 {
                let hints = if self.popups.active() {
                    Some(Line::from(vec![
                        " ".into(),
                        "↑↓".bold(),
                        Span::styled(" navigate · ", secondary_style()),
                        key_hint::plain(KeyCode::Esc).into(),
                        Span::styled(" close ", secondary_style()),
                    ]))
                } else if self.draft.is_bash_mode {
                    Some(Line::from(Span::styled(
                        " ! shell command ",
                        secondary_style(),
                    )))
                } else if area.width < 60
                    && let Some(binding) = self.footer.show_transcript_key
                {
                    // Compact conversations frequently begin above the visible terminal.
                    // Keep the real transcript action discoverable without repainting history.
                    Some(Line::from(vec![
                        " ".into(),
                        binding.into(),
                        Span::styled(" transcript ", secondary_style()),
                    ]))
                } else {
                    None
                };
                if let Some(hints) = hints {
                    frame = frame.title_bottom(hints.style(hint_style));
                }
            }
        }
        let inner = frame.inner(area);
        Block::default().style(surface).render(inner, buf);
        frame.render(area, buf);
        if self.has_focus
            && self.draft.input_enabled
            && !self.blocks_direct_input
            && !area.is_empty()
        {
            for y in area.top()..area.bottom() {
                let cell = &mut buf[(area.left(), y)];
                cell.set_style(accent_style());
                if cell.symbol() == "│" {
                    cell.set_symbol("┃");
                }
            }
        }
        surface
    }
}
