//! Keep the fork's bounded input preview while questions own the queue-navigation hint.

use super::PendingInputPreview;
use super::VISIBLE_ROW_CAP;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum QuestionPresence {
    Absent,
    Present,
}

/// Questions provide their own navigation hint; queued inputs retain their row budget.
pub(in crate::bottom_pane) struct PendingInputPreviewContent<'a>(
    pub(in crate::bottom_pane) &'a PendingInputPreview,
);

impl Renderable for PendingInputPreviewContent<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.0
            .render_with_questions(area, buf, QuestionPresence::Present);
    }

    fn desired_height(&self, width: u16) -> u16 {
        u16::try_from(
            self.0
                .preview_lines(width, VISIBLE_ROW_CAP, QuestionPresence::Present)
                .len(),
        )
        .unwrap_or(u16::MAX)
    }
}
