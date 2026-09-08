//! End-to-end coverage for diff rendering under a simulated light terminal palette.
//!
//! `diff_render.rs`'s own inline test module already unit-tests the style-resolution functions
//! (`style_line_bg_for`, `resolve_diff_backgrounds_for`, ...) directly with an explicit
//! `DiffTheme::Light`. What it does not cover is the *public* entry point
//! (`create_diff_summary`) actually picking up light colors end-to-end through the same terminal
//! background detection the app uses at runtime. `crate::terminal_palette::with_test_default_colors`
//! is the "owned test helper" for that: it scopes a real light background for the duration of a
//! render, which also makes `effective_stdout_color_level()` report true color (see
//! `terminal_palette.rs`), so this test exercises the exact GitHub-style light pastels a user would
//! see rather than the ANSI-16 foreground-only fallback used when no palette is known.

use super::*;
use crate::diff_model::FileChange;
use crate::terminal_palette::rgb_color;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use std::collections::HashMap;
use std::path::PathBuf;

/// Matches the light-palette convention used by `chat_composer.rs`'s
/// `light_terminal_palette_renders_light_composer_snapshot` test.
const LIGHT_TERMINAL_COLORS: DefaultColors = DefaultColors {
    fg: (0x55, 0x57, 0x53),
    bg: (0xff, 0xff, 0xff),
};

fn light_palette_changes() -> HashMap<PathBuf, FileChange> {
    let original = "def greet(name):\n    print(\"hi \" + name)\n";
    let modified = "def greet(name):\n    print(f\"hi {name}!\")\n";
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("greet.py"),
        FileChange::Update {
            unified_diff: diffy::create_patch(original, modified).to_string(),
            move_path: None,
        },
    );
    changes
}

#[test]
fn light_palette_diff_summary_uses_github_style_pastels_end_to_end() {
    let (buffer, add_bg, del_bg) = with_test_default_colors(LIGHT_TERMINAL_COLORS, || {
        let width = 60u16;
        let lines =
            create_diff_summary(&light_palette_changes(), &PathBuf::from("/"), width.into());
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 10);
        let mut buffer = Buffer::empty(area);
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .render(area, &mut buffer);
        (
            buffer,
            rgb_color(LIGHT_TC_ADD_LINE_BG_RGB),
            rgb_color(LIGHT_TC_DEL_LINE_BG_RGB),
        )
    });

    let backgrounds: std::collections::HashSet<_> =
        buffer.content().iter().map(|cell| cell.bg).collect();
    assert!(
        backgrounds.contains(&add_bg),
        "expected the light add-line pastel {add_bg:?} somewhere in the rendered diff: {buffer:?}",
    );
    assert!(
        backgrounds.contains(&del_bg),
        "expected the light delete-line pastel {del_bg:?} somewhere in the rendered diff: {buffer:?}",
    );
    assert_ne!(
        add_bg, del_bg,
        "light add/delete pastels must stay visually distinct",
    );

    insta::assert_snapshot!("light_palette_diff_summary", format!("{buffer:?}"));
}
