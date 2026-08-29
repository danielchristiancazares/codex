use ratatui::layout::Rect;

const FULL_CONTROLS_HEIGHT: u16 = 5;
const COMPACT_CONTROLS_HEIGHT: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChoiceVisibility {
    Hidden,
    Visible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OssSelectionLayout {
    pub(super) prompt_area: Rect,
    pub(super) title_area: Option<Rect>,
    pub(super) button_area: Option<Rect>,
    pub(super) description_area: Option<Rect>,
}

impl OssSelectionLayout {
    pub(super) fn new(area: Rect, prompt_preferred_height: u16) -> Self {
        if area.width == 0 || area.height == 0 {
            return Self {
                prompt_area: Rect::new(area.x, area.y, area.width, 0),
                title_area: None,
                button_area: None,
                description_area: None,
            };
        }

        let use_full_controls =
            area.height >= prompt_preferred_height.saturating_add(FULL_CONTROLS_HEIGHT);
        let controls_height = if use_full_controls {
            FULL_CONTROLS_HEIGHT
        } else {
            area.height.min(COMPACT_CONTROLS_HEIGHT)
        };
        let prompt_height = area.height.saturating_sub(controls_height);
        let controls_y = area.y.saturating_add(prompt_height);
        let row = |offset| Rect::new(area.x, controls_y.saturating_add(offset), area.width, 1);

        let (title_area, button_area, description_area) = if use_full_controls {
            (Some(row(1)), Some(row(2)), Some(row(3)))
        } else {
            match controls_height {
                1 => (None, Some(row(0)), None),
                2 => (None, Some(row(0)), Some(row(1))),
                3 => (Some(row(0)), Some(row(1)), Some(row(2))),
                _ => (None, None, None),
            }
        };

        Self {
            prompt_area: Rect::new(area.x, area.y, area.width, prompt_height),
            title_area,
            button_area,
            description_area,
        }
    }

    pub(super) fn controls_visibility(self) -> ChoiceVisibility {
        if self.button_area.is_some() {
            ChoiceVisibility::Visible
        } else {
            ChoiceVisibility::Hidden
        }
    }
}

#[cfg(test)]
#[path = "oss_selection_layout_tests.rs"]
mod tests;
