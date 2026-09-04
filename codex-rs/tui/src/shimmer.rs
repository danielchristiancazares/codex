use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;

use crate::color::strengthen_contrast;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::effective_stdout_color_level;
use crate::terminal_palette::rgb_color;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();
const SHIMMER_PADDING: usize = 8;
const SHIMMER_SWEEP_SECONDS: f32 = 2.4;
const SHIMMER_BAND_HALF_WIDTH: f32 = 4.0;
const SHIMMER_CONTRAST_BOOST: f32 = 0.28;

fn elapsed_since_start() -> Duration {
    let start = PROCESS_START.get_or_init(Instant::now);
    start.elapsed()
}

pub(super) fn shimmer_spans(text: &str) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let period = chars.len() + SHIMMER_PADDING * 2;
    let position = ((elapsed_since_start().as_secs_f32() % SHIMMER_SWEEP_SECONDS)
        / SHIMMER_SWEEP_SECONDS
        * period as f32) as usize;
    shimmer_spans_at_position(
        chars,
        position,
        default_fg().zip(default_bg()),
        effective_stdout_color_level(),
    )
}

fn shimmer_spans_at_position(
    chars: Vec<char>,
    position: usize,
    terminal_colors: Option<((u8, u8, u8), (u8, u8, u8))>,
    color_level: StdoutColorLevel,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(chars.len());
    for (index, ch) in chars.into_iter().enumerate() {
        let char_position = index as isize + SHIMMER_PADDING as isize;
        let distance = (char_position - position as isize).abs() as f32;

        let intensity = if distance <= SHIMMER_BAND_HALF_WIDTH {
            let x = std::f32::consts::PI * (distance / SHIMMER_BAND_HALF_WIDTH);
            0.5 * (1.0 + x.cos())
        } else {
            0.0
        };
        let style = match (color_level, terminal_colors) {
            (StdoutColorLevel::TrueColor, Some((foreground, background))) => {
                truecolor_style(intensity, foreground, background)
            }
            (
                StdoutColorLevel::TrueColor
                | StdoutColorLevel::Ansi256
                | StdoutColorLevel::Ansi16
                | StdoutColorLevel::Unknown,
                _,
            ) => emphasis_for_level(intensity),
        };
        spans.push(Span::styled(ch.to_string(), style));
    }
    spans
}

fn truecolor_style(intensity: f32, foreground: (u8, u8, u8), background: (u8, u8, u8)) -> Style {
    if intensity <= f32::EPSILON {
        return Style::default();
    }

    let color = strengthen_contrast(
        foreground,
        background,
        intensity.clamp(0.0, 1.0) * SHIMMER_CONTRAST_BOOST,
    );
    let style = Style::default().fg(rgb_color(color));
    if intensity >= 0.90 {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

fn emphasis_for_level(intensity: f32) -> Style {
    if intensity >= 0.90 {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;

    use super::*;
    use crate::color::contrast_ratio;

    #[test]
    fn shimmer_snapshot_preserves_legibility_across_color_levels() {
        let chars = || "Working".chars().collect::<Vec<_>>();
        let position = SHIMMER_PADDING + 3;
        let cases = [
            (
                "dark truecolor",
                shimmer_spans_at_position(
                    chars(),
                    position,
                    Some(((218, 220, 224), (15, 17, 20))),
                    StdoutColorLevel::TrueColor,
                ),
            ),
            (
                "light truecolor",
                shimmer_spans_at_position(
                    chars(),
                    position,
                    Some(((45, 48, 52), (246, 246, 242))),
                    StdoutColorLevel::TrueColor,
                ),
            ),
            (
                "256-color fallback",
                shimmer_spans_at_position(
                    chars(),
                    position,
                    Some(((218, 220, 224), (15, 17, 20))),
                    StdoutColorLevel::Ansi256,
                ),
            ),
            (
                "16-color fallback",
                shimmer_spans_at_position(
                    chars(),
                    position,
                    Some(((218, 220, 224), (15, 17, 20))),
                    StdoutColorLevel::Ansi16,
                ),
            ),
            (
                "no-color fallback",
                shimmer_spans_at_position(
                    chars(),
                    position,
                    /*terminal_colors*/ None,
                    StdoutColorLevel::Unknown,
                ),
            ),
        ];

        insta::assert_debug_snapshot!("shimmer_across_terminal_capabilities", cases);
    }

    #[test]
    fn truecolor_shimmer_never_weakens_foreground_contrast() {
        for (foreground, background) in [
            ((218, 220, 224), (15, 17, 20)),
            ((45, 48, 52), (246, 246, 242)),
        ] {
            let spans = shimmer_spans_at_position(
                "Working".chars().collect(),
                SHIMMER_PADDING + 3,
                Some((foreground, background)),
                StdoutColorLevel::TrueColor,
            );
            let highlighted = spans[3].style.fg.expect("highlight foreground");
            let Color::Rgb(r, g, b) = highlighted else {
                panic!("expected derived RGB highlight, got {highlighted:?}");
            };
            assert!(
                contrast_ratio((r, g, b), background) >= contrast_ratio(foreground, background)
            );
        }
    }

    #[test]
    fn truecolor_without_detected_defaults_uses_modifier_only_fallback() {
        let spans = shimmer_spans_at_position(
            "Busy".chars().collect(),
            SHIMMER_PADDING + 2,
            /*terminal_colors*/ None,
            StdoutColorLevel::TrueColor,
        );

        assert_eq!(
            spans.iter().map(|span| span.style.fg).collect::<Vec<_>>(),
            vec![None; 4]
        );
    }
}
