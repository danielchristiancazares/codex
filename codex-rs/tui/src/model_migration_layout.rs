use ratatui::layout::Rect;

const FULL_MENU_HEIGHT: u16 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChoiceVisibility {
    Hidden,
    Visible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ModelMigrationLayout {
    pub(super) copy_area: Rect,
    pub(super) compact_options_area: Option<Rect>,
    pub(super) instruction_area: Option<Rect>,
    pub(super) option_areas: [Option<Rect>; 2],
    pub(super) guidance_area: Option<Rect>,
    pub(super) full_guidance: bool,
}

impl ModelMigrationLayout {
    pub(super) fn new(area: Rect, copy_preferred_height: u16) -> Self {
        if area.width == 0 || area.height == 0 {
            return Self::empty(area);
        }

        let preserve_full_layout =
            area.height >= copy_preferred_height.saturating_add(FULL_MENU_HEIGHT);
        let menu_height = if preserve_full_layout {
            FULL_MENU_HEIGHT
        } else {
            area.height.min(FULL_MENU_HEIGHT)
        };
        let copy_height = if preserve_full_layout {
            copy_preferred_height
        } else {
            area.height.saturating_sub(menu_height)
        };
        let menu_y = area.y.saturating_add(copy_height);
        let row = |offset| Rect::new(area.x, menu_y.saturating_add(offset), area.width, 1);

        let mut layout = Self {
            copy_area: Rect::new(area.x, area.y, area.width, copy_height),
            compact_options_area: None,
            instruction_area: None,
            option_areas: [None, None],
            guidance_area: None,
            full_guidance: menu_height == FULL_MENU_HEIGHT,
        };
        match menu_height {
            1 => layout.compact_options_area = Some(row(0)),
            2 => layout.option_areas = [Some(row(0)), Some(row(1))],
            3 => {
                layout.option_areas = [Some(row(0)), Some(row(1))];
                layout.guidance_area = Some(row(2));
            }
            4 => {
                layout.instruction_area = Some(row(0));
                layout.option_areas = [Some(row(1)), Some(row(2))];
                layout.guidance_area = Some(row(3));
            }
            5 => {
                layout.instruction_area = Some(row(0));
                layout.option_areas = [Some(row(2)), Some(row(3))];
                layout.guidance_area = Some(row(4));
            }
            6 => {
                layout.instruction_area = Some(row(0));
                layout.option_areas = [Some(row(2)), Some(row(3))];
                layout.guidance_area = Some(row(5));
            }
            7 => {
                layout.instruction_area = Some(row(1));
                layout.option_areas = [Some(row(3)), Some(row(4))];
                layout.guidance_area = Some(row(6));
            }
            _ => {}
        }
        layout
    }

    pub(super) fn choice_visibility(self) -> ChoiceVisibility {
        if self.compact_options_area.is_some() || self.option_areas.iter().any(Option::is_some) {
            ChoiceVisibility::Visible
        } else {
            ChoiceVisibility::Hidden
        }
    }

    fn empty(area: Rect) -> Self {
        Self {
            copy_area: Rect::new(area.x, area.y, area.width, 0),
            compact_options_area: None,
            instruction_area: None,
            option_areas: [None, None],
            guidance_area: None,
            full_guidance: false,
        }
    }
}

#[cfg(test)]
#[path = "model_migration_layout_tests.rs"]
mod tests;
