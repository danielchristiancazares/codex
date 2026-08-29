use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PagerFrameLayout {
    pub(super) frame_area: Rect,
    pub(super) title_area: Option<Rect>,
    pub(super) content_area: Rect,
    pub(super) separator_area: Option<Rect>,
    pub(super) navigation_hint_area: Option<Rect>,
    pub(super) close_hint_area: Option<Rect>,
    pub(super) trailing_spacer_area: Option<Rect>,
}

impl PagerFrameLayout {
    pub(super) fn new(area: Rect) -> Self {
        let row = |offset| Rect::new(area.x, area.y.saturating_add(offset), area.width, 1);
        match area.height {
            0 => Self {
                frame_area: area,
                title_area: None,
                content_area: Rect::new(area.x, area.y, area.width, 0),
                separator_area: None,
                navigation_hint_area: None,
                close_hint_area: None,
                trailing_spacer_area: None,
            },
            1 => Self {
                frame_area: area,
                title_area: None,
                content_area: row(0),
                separator_area: None,
                navigation_hint_area: None,
                close_hint_area: None,
                trailing_spacer_area: None,
            },
            2 => Self {
                frame_area: area,
                title_area: None,
                content_area: row(0),
                separator_area: None,
                navigation_hint_area: None,
                close_hint_area: Some(row(1)),
                trailing_spacer_area: None,
            },
            3 => Self {
                frame_area: area,
                title_area: Some(row(0)),
                content_area: row(1),
                separator_area: None,
                navigation_hint_area: None,
                close_hint_area: Some(row(2)),
                trailing_spacer_area: None,
            },
            4 => Self {
                frame_area: area,
                title_area: Some(row(0)),
                content_area: row(1),
                separator_area: None,
                navigation_hint_area: Some(row(2)),
                close_hint_area: Some(row(3)),
                trailing_spacer_area: None,
            },
            5 => Self {
                frame_area: area,
                title_area: Some(row(0)),
                content_area: row(1),
                separator_area: Some(row(2)),
                navigation_hint_area: Some(row(3)),
                close_hint_area: Some(row(4)),
                trailing_spacer_area: None,
            },
            height => Self {
                frame_area: area,
                title_area: Some(row(0)),
                content_area: Rect::new(
                    area.x,
                    area.y.saturating_add(1),
                    area.width,
                    height.saturating_sub(5),
                ),
                separator_area: Some(row(height.saturating_sub(4))),
                navigation_hint_area: Some(row(height.saturating_sub(3))),
                close_hint_area: Some(row(height.saturating_sub(2))),
                trailing_spacer_area: Some(row(height.saturating_sub(1))),
            },
        }
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
