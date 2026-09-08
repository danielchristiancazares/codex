//! Top-aligned startup identity and progress remain visible when only one header row fits.

use super::StartupDraftSessionAction;
use crate::bottom_pane::BottomPane;
use crate::history_cell::HistoryCell;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableItem;
use crate::start_screen::StartScreen;
use crate::start_screen::StartScreenPresentation;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

pub(super) fn startup_draft_renderable<'a>(
    header: &'a dyn HistoryCell,
    bottom_pane: &'a BottomPane,
    session_action: StartupDraftSessionAction,
) -> RenderableItem<'a> {
    RenderableItem::Owned(Box::new(StartScreen {
        presentation: StartScreenPresentation::Introduction,
        header: RenderableItem::Owned(Box::new(StartupHeader {
            header,
            session_action,
        })),
        composer: bottom_pane
            .as_renderable_with_composer_right_reserve(/*composer_right_reserve*/ 0),
        actions: RenderableItem::Owned(Box::new(())),
    }))
}

struct StartupHeader<'a> {
    header: &'a dyn HistoryCell,
    session_action: StartupDraftSessionAction,
}

impl Renderable for StartupHeader<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let progress = match self.session_action {
            StartupDraftSessionAction::New => None,
            StartupDraftSessionAction::Resume => Some("Resuming session…"),
            StartupDraftSessionAction::Fork => Some("Forking session…"),
        };
        let mut lines = self.header.display_lines(area.width);
        if let Some(progress) = progress {
            if area.height == 1 {
                lines = vec![Line::from(progress.bold())];
            } else {
                lines.push(Line::default());
                lines.push(Line::from(progress.bold()));
            }
        }
        Paragraph::new(lines).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.header.desired_height(width).saturating_add(2)
    }
}
