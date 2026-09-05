use crate::color::blend;
use crate::color::is_light;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::best_color;
use crate::terminal_palette::best_color_for_level;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::effective_stdout_color_level;
use crate::terminal_palette::rgb_color;
use ratatui::style::Color;
use ratatui::style::Style;

const LIGHT_BG_ACCENT_RGB: (u8, u8, u8) = (132, 0, 120);

#[cfg(test)]
#[path = "style_contrast_tests.rs"]
mod contrast_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatusTone {
    Success,
    Attention,
    Failure,
}

/// Semantic status colors that preserve the terminal's configured palette.
pub(crate) fn status_style(tone: StatusTone) -> Style {
    status_style_for(tone, effective_stdout_color_level())
}

fn status_style_for(tone: StatusTone, color_level: StdoutColorLevel) -> Style {
    let color = match tone {
        StatusTone::Success => Color::Green,
        StatusTone::Attention => Color::Reset,
        StatusTone::Failure => Color::Red,
    };
    emphasized_ansi_style(color, color_level)
}

/// Returns the low-emphasis style for supporting copy and metadata.
pub(crate) fn secondary_style() -> Style {
    let level = effective_stdout_color_level();
    if matches!(
        level,
        StdoutColorLevel::TrueColor | StdoutColorLevel::Ansi256
    ) && let (Some(fg), Some(bg)) = (default_fg(), default_bg())
    {
        Style::default().fg(best_color_for_level(blend(fg, bg, /*alpha*/ 0.70), level))
    } else {
        Style::default()
    }
}

/// Returns the high-contrast, non-color-dependent style for shortcut tokens.
pub(crate) fn key_hint_style() -> Style {
    Style::default().fg(Color::Reset).bold().not_dim()
}

/// Returns the Codex identity style used by a single meaningful visual mark.
pub(crate) fn brand_style() -> Style {
    brand_style_for(effective_stdout_color_level())
}

fn brand_style_for(color_level: StdoutColorLevel) -> Style {
    emphasized_ansi_style(Color::Magenta, color_level)
}

fn emphasized_ansi_style(color: Color, color_level: StdoutColorLevel) -> Style {
    let color = if color_level == StdoutColorLevel::Unknown {
        Color::Reset
    } else {
        color
    };
    Style::default().fg(color).bold()
}
// Decorative table rules should remain visible without competing with cell content.
const TABLE_SEPARATOR_FG_ALPHA: f32 = 0.20;

pub fn user_message_style() -> Style {
    user_message_style_for(default_bg())
}

pub fn proposed_plan_style() -> Style {
    proposed_plan_style_for(default_bg())
}

/// Returns a low-contrast rule style for separators within markdown tables.
pub(crate) fn table_separator_style() -> Style {
    table_separator_style_for(default_fg(), default_bg(), effective_stdout_color_level())
}

/// Returns the shared accent style for active or selected TUI controls.
pub(crate) fn accent_style() -> Style {
    accent_style_for(default_bg(), effective_stdout_color_level())
}

/// Returns the shared chip style for image attachments and references.
pub(crate) fn attachment_chip_style() -> Style {
    attachment_chip_style_for(default_bg(), effective_stdout_color_level())
}

/// Returns the flat style for a user-authored message.
pub fn user_message_style_for(_terminal_bg: Option<(u8, u8, u8)>) -> Style {
    Style::default()
}

pub fn proposed_plan_style_for(_terminal_bg: Option<(u8, u8, u8)>) -> Style {
    Style::default()
}

/// Returns the shared accent style for the provided terminal background.
fn accent_style_for(terminal_bg: Option<(u8, u8, u8)>, color_level: StdoutColorLevel) -> Style {
    match color_level {
        StdoutColorLevel::Unknown => emphasized_ansi_style(Color::Reset, color_level),
        StdoutColorLevel::Ansi16 if terminal_bg.is_some_and(is_light) => {
            emphasized_ansi_style(Color::Reset, color_level)
        }
        StdoutColorLevel::TrueColor | StdoutColorLevel::Ansi256
            if terminal_bg.is_some_and(is_light) =>
        {
            Style::default()
                .fg(best_color_for_level(LIGHT_BG_ACCENT_RGB, color_level))
                .bold()
        }
        StdoutColorLevel::TrueColor | StdoutColorLevel::Ansi256 | StdoutColorLevel::Ansi16 => {
            emphasized_ansi_style(Color::Magenta, color_level)
        }
    }
}

fn attachment_chip_style_for(
    terminal_bg: Option<(u8, u8, u8)>,
    color_level: StdoutColorLevel,
) -> Style {
    accent_style_for(terminal_bg, color_level)
}

fn table_separator_style_for(
    terminal_fg: Option<(u8, u8, u8)>,
    terminal_bg: Option<(u8, u8, u8)>,
    color_level: StdoutColorLevel,
) -> Style {
    let (Some(fg), Some(bg)) = (terminal_fg, terminal_bg) else {
        return Style::default().dim();
    };
    let separator_rgb = blend(fg, bg, TABLE_SEPARATOR_FG_ALPHA);
    match color_level {
        StdoutColorLevel::TrueColor => Style::default().fg(rgb_color(separator_rgb)),
        StdoutColorLevel::Ansi256 => Style::default().fg(best_color(separator_rgb)),
        StdoutColorLevel::Ansi16 | StdoutColorLevel::Unknown => Style::default().dim(),
    }
}

pub(crate) fn user_message_bg_rgb(terminal_bg: (u8, u8, u8)) -> (u8, u8, u8) {
    let (top, alpha) = if is_light(terminal_bg) {
        ((0, 0, 0), 0.04)
    } else {
        ((255, 255, 255), 0.12)
    };
    blend(top, terminal_bg, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    #[test]
    fn status_styles_use_color_and_text_redundancy() {
        for level in [
            StdoutColorLevel::TrueColor,
            StdoutColorLevel::Ansi256,
            StdoutColorLevel::Ansi16,
        ] {
            assert_eq!(
                [
                    status_style_for(StatusTone::Success, level),
                    status_style_for(StatusTone::Attention, level),
                    status_style_for(StatusTone::Failure, level),
                ],
                [
                    Style::default().fg(Color::Green).bold(),
                    Style::default().fg(Color::Reset).bold(),
                    Style::default().fg(Color::Red).bold(),
                ]
            );
        }
        assert_eq!(
            [
                status_style_for(StatusTone::Success, StdoutColorLevel::Unknown),
                status_style_for(StatusTone::Attention, StdoutColorLevel::Unknown),
                status_style_for(StatusTone::Failure, StdoutColorLevel::Unknown),
            ],
            [Style::default().fg(Color::Reset).bold(); 3]
        );
    }

    #[test]
    fn brand_accent_and_key_hint_styles_adapt_without_losing_emphasis() {
        assert_eq!(
            [
                brand_style_for(StdoutColorLevel::TrueColor),
                brand_style_for(StdoutColorLevel::Unknown),
                accent_style_for(Some((0, 0, 0)), StdoutColorLevel::TrueColor),
                accent_style_for(Some((255, 255, 255)), StdoutColorLevel::TrueColor),
                accent_style_for(Some((0, 0, 0)), StdoutColorLevel::Ansi16),
                accent_style_for(Some((255, 255, 255)), StdoutColorLevel::Ansi16),
                accent_style_for(Some((0, 0, 0)), StdoutColorLevel::Unknown),
            ],
            [
                Style::default().fg(Color::Magenta).bold(),
                Style::default().fg(Color::Reset).bold(),
                Style::default().fg(Color::Magenta).bold(),
                Style::default()
                    .fg(best_color_for_level(
                        LIGHT_BG_ACCENT_RGB,
                        StdoutColorLevel::TrueColor
                    ))
                    .bold(),
                Style::default().fg(Color::Magenta).bold(),
                Style::default().fg(Color::Reset).bold(),
                Style::default().fg(Color::Reset).bold(),
            ]
        );
        assert!(key_hint_style().add_modifier.contains(Modifier::BOLD));
        assert!(key_hint_style().sub_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn attachment_chip_style_uses_the_shared_accent_without_a_fill() {
        assert_eq!(
            attachment_chip_style_for(Some((0, 0, 0)), StdoutColorLevel::TrueColor),
            Style::default().fg(Color::Magenta).bold()
        );
        assert_eq!(
            attachment_chip_style_for(Some((255, 255, 255)), StdoutColorLevel::TrueColor),
            Style::default()
                .fg(best_color_for_level(
                    LIGHT_BG_ACCENT_RGB,
                    StdoutColorLevel::TrueColor
                ))
                .bold()
        );
    }

    #[test]
    fn attachment_chip_fallbacks_follow_the_shared_accent_role() {
        assert_eq!(
            [
                attachment_chip_style_for(/*terminal_bg*/ None, StdoutColorLevel::TrueColor),
                attachment_chip_style_for(Some((0, 0, 0)), StdoutColorLevel::Ansi16),
                attachment_chip_style_for(Some((255, 255, 255)), StdoutColorLevel::Ansi16),
                attachment_chip_style_for(Some((255, 255, 255)), StdoutColorLevel::Unknown),
            ],
            [
                Style::default().fg(Color::Magenta).bold(),
                Style::default().fg(Color::Magenta).bold(),
                Style::default().fg(Color::Reset).bold(),
                Style::default().fg(Color::Reset).bold(),
            ]
        );
    }

    #[test]
    fn table_separator_blends_toward_dark_background() {
        let style = table_separator_style_for(
            Some((255, 255, 255)),
            Some((0, 0, 0)),
            StdoutColorLevel::TrueColor,
        );

        assert_eq!(style.fg, Some(rgb_color((51, 51, 51))));
    }

    #[test]
    fn table_separator_blends_toward_light_background() {
        let style = table_separator_style_for(
            Some((0, 0, 0)),
            Some((255, 255, 255)),
            StdoutColorLevel::TrueColor,
        );

        assert_eq!(style.fg, Some(rgb_color((204, 204, 204))));
    }

    #[test]
    fn table_separator_dims_when_palette_aware_color_is_unavailable() {
        let expected = Style::default().dim();

        assert_eq!(
            table_separator_style_for(
                Some((255, 255, 255)),
                Some((0, 0, 0)),
                StdoutColorLevel::Ansi16,
            ),
            expected
        );
        assert_eq!(
            table_separator_style_for(
                /*terminal_fg*/ None,
                Some((0, 0, 0)),
                StdoutColorLevel::TrueColor,
            ),
            expected
        );
    }

    #[test]
    fn semantic_styles_snapshot_across_terminal_capabilities() {
        let fixtures = [
            (
                "dark truecolor",
                Some((12, 15, 18)),
                StdoutColorLevel::TrueColor,
            ),
            (
                "light truecolor",
                Some((245, 245, 240)),
                StdoutColorLevel::TrueColor,
            ),
            (
                "dark 256-color",
                Some((12, 15, 18)),
                StdoutColorLevel::Ansi256,
            ),
            (
                "light 256-color",
                Some((245, 245, 240)),
                StdoutColorLevel::Ansi256,
            ),
            (
                "dark 16-color",
                Some((12, 15, 18)),
                StdoutColorLevel::Ansi16,
            ),
            (
                "light 16-color",
                Some((245, 245, 240)),
                StdoutColorLevel::Ansi16,
            ),
            ("no color", None, StdoutColorLevel::Unknown),
        ]
        .map(|(environment, terminal_bg, color_level)| {
            (
                environment,
                [
                    ("brand", brand_style_for(color_level)),
                    ("accent", accent_style_for(terminal_bg, color_level)),
                    (
                        "success",
                        status_style_for(StatusTone::Success, color_level),
                    ),
                    (
                        "attention",
                        status_style_for(StatusTone::Attention, color_level),
                    ),
                    (
                        "failure",
                        status_style_for(StatusTone::Failure, color_level),
                    ),
                    ("key hint", key_hint_style()),
                    ("secondary", secondary_style()),
                    (
                        "attachment",
                        attachment_chip_style_for(terminal_bg, color_level),
                    ),
                ],
            )
        });

        insta::assert_debug_snapshot!("semantic_styles_across_terminal_capabilities", fixtures);
    }
}
