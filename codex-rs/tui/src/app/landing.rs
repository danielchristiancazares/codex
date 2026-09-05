//! Empty-session framing uses the same full-height inline presentation as the existing dashboard.
//! History remains authoritative and is restored by the normal full-height-to-inline reflow path.

use super::*;

impl App {
    pub(super) fn render_landing_frame(
        &mut self,
        tui: &mut tui::Tui,
        screen_size: Size,
    ) -> Result<Option<Rect>> {
        if self.transcript_cells.is_empty()
            || self.scrollback_has_older_history
            || !self
                .transcript_cells
                .iter()
                .all(|cell| cell.as_any().is::<history_cell::SessionInfoCell>())
        {
            return Ok(None);
        }
        let Some(surface) = self.chat_widget.landing_surface() else {
            return Ok(None);
        };
        let mut rendered_area = Rect::default();
        tui.draw_with_resize_reflow(
            screen_size.height,
            screen_size,
            tui::InlineViewportPlacement::FollowExisting,
            tui::InlineViewportRole::Transient,
            |frame| {
                let area = frame.area();
                rendered_area = area;
                surface.render(area, frame.buffer);
                self.chat_widget.note_rendered_width(area.width);
                if let Some((x, y)) = surface.cursor_pos(area) {
                    frame.set_cursor_style(surface.cursor_style(area));
                    frame.set_cursor_position((x, y));
                }
            },
        )?;
        Ok(Some(rendered_area))
    }
}

#[cfg(test)]
#[path = "landing_tests.rs"]
mod tests;
