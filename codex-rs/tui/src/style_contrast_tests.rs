//! Essential supporting text stays readable across the palettes used in the visual review.

use super::*;
use crate::terminal_palette::with_test_terminal_palette;
use crate::terminal_probe::DefaultColors;
use pretty_assertions::assert_eq;

fn luminance(rgb: (u8, u8, u8)) -> f64 {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb.0) + 0.7152 * linear(rgb.1) + 0.0722 * linear(rgb.2)
}

#[test]
fn supporting_text_retains_contrast_on_light_and_dark_palettes() {
    let mut snapshots = Vec::new();
    for (name, fg, bg) in [
        ("dark", (220, 220, 216), (32, 32, 32)),
        ("light", (36, 36, 36), (250, 249, 246)),
    ] {
        with_test_terminal_palette(
            DefaultColors { fg, bg },
            StdoutColorLevel::TrueColor,
            || {
                let style = secondary_style();
                let Some(Color::Rgb(r, g, b)) = style.fg else {
                    panic!("expected palette-derived text color");
                };
                let foreground = luminance((r, g, b));
                let background = luminance(bg);
                let contrast =
                    (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05);
                assert!(
                    contrast >= 4.5,
                    "{name} supporting text contrast: {contrast}"
                );
                assert!(!style.add_modifier.contains(ratatui::style::Modifier::DIM));
                snapshots.push(format!("{name}: {style:?}, contrast {contrast:.2}"));
            },
        );
        for level in [StdoutColorLevel::Ansi16, StdoutColorLevel::Unknown] {
            with_test_terminal_palette(DefaultColors { fg, bg }, level, || {
                assert_eq!(secondary_style(), Style::default());
            });
        }
    }
    insta::assert_snapshot!("supporting_text_contrast", snapshots.join("\n"));
}
