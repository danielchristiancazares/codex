//! Shared framing for startup and an empty session. Both keep the real composer in the same
//! bounded column, reserving space for its ready-state footer so hydration does not move input.

use crossterm::cursor::SetCursorStyle;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableItem;

const COLUMN_WIDTH: u16 = 88;
const COMPOSER_MIN_HEIGHT: u16 = 6;

pub(crate) struct StartScreen<'a> {
    pub header: RenderableItem<'a>,
    pub composer: RenderableItem<'a>,
    pub actions: RenderableItem<'a>,
}

impl StartScreen<'_> {
    fn column_width(width: u16) -> u16 {
        width
            .saturating_sub(if width >= 40 { 4 } else { 0 })
            .min(COLUMN_WIDTH)
    }

    fn layout(&self, area: Rect) -> (Rect, Rect, Rect) {
        let width = Self::column_width(area.width);
        let composer_height = self
            .composer
            .desired_height(width)
            .max(COMPOSER_MIN_HEIGHT)
            .min(area.height);
        let header_height = if area.height >= composer_height.saturating_add(10) {
            7
        } else if area.height >= composer_height.saturating_add(2) {
            1
        } else {
            0
        };
        let actions_height = if area.height
            >= composer_height
                .saturating_add(header_height)
                .saturating_add(4)
        {
            4
        } else {
            0
        };
        let total_height = composer_height
            .saturating_add(header_height)
            .saturating_add(actions_height);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(total_height) / 2;
        let header = Rect::new(x, y, width, header_height);
        let composer = Rect::new(x, header.bottom(), width, composer_height);
        let actions = Rect::new(
            x + width.min(2),
            composer.bottom() + actions_height.min(1),
            width.saturating_sub(4),
            actions_height.saturating_sub(1),
        );
        (header, composer, actions)
    }
}

impl Renderable for StartScreen<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let (header, composer, actions) = self.layout(area);
        if !header.is_empty() {
            self.header.render(header, buf);
        }
        self.composer.render(composer, buf);
        if !actions.is_empty() {
            self.actions.render(actions, buf);
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.composer
            .desired_height(Self::column_width(width))
            .max(COMPOSER_MIN_HEIGHT)
            .saturating_add(11)
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.composer.cursor_pos(self.layout(area).1)
    }

    fn cursor_style(&self, area: Rect) -> SetCursorStyle {
        self.composer.cursor_style(self.layout(area).1)
    }
}
