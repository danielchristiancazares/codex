//! A compact terminal-native wordmark for the empty workspace. Small viewports use the same
//! textual identity as session history; active conversations retain their ordinary working grid.

use crate::render::renderable::Renderable;
use crate::style::accent_style;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

pub(crate) struct StartScreenIdentity;

impl Renderable for StartScreenIdentity {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height >= 3 && area.width >= 40 {
            for (row, (mark, name)) in [
                ("▄▀▀▀▀", "  ▄▀▀▀▄  █▀▀▀▄  █▀▀▀▀  ▀▄ ▄▀"),
                ("█    ", "  █   █  █   █  █▀▀▀    ▄▀▄ "),
                (" ▀▀▀▀", "   ▀▀▀   ▀▀▀▀   ▀▀▀▀▀  ▀   ▀"),
            ]
            .into_iter()
            .enumerate()
            {
                Line::from(vec![Span::styled(mark, accent_style()), name.bold()]).render(
                    Rect::new(area.x, area.y + row as u16, area.width, /*height*/ 1),
                    buf,
                );
            }
        } else {
            Line::from(vec![Span::styled(">_ ", accent_style()), "Codex".bold()]).render(area, buf);
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        if width >= 40 { 3 } else { 1 }
    }
}
