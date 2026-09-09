use super::*;

#[test]
fn selected_description_reflows_when_the_list_cannot_show_it() {
    let mut snapshots = Vec::new();
    for width in [24, 48, 80, 120] {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        popup.on_composer_text_change("/ide".to_string());
        let height = popup.calculate_required_height(width);
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buffer = Buffer::empty(area);
        crate::terminal_palette::with_test_terminal_palette(
            crate::terminal_probe::DefaultColors {
                fg: (220, 220, 216),
                bg: (32, 32, 32),
            },
            crate::terminal_palette::StdoutColorLevel::Ansi16,
            || popup.render_ref(area, &mut buffer),
        );
        let text = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("/ide"), "{text}");
        if width >= 48 {
            let visible_description = text.lines().map(str::trim).collect::<Vec<_>>().join(" ");
            assert!(
                visible_description.contains(
                    popup
                        .selected_item()
                        .expect("selected command")
                        .description()
                ),
                "{text}"
            );
        }
        if width == 120 {
            pretty_assertions::assert_eq!(
                text.matches(
                    popup
                        .selected_item()
                        .expect("selected command")
                        .description()
                )
                .count(),
                1
            );
        }
        snapshots.push(format!("{width} columns\n{buffer:?}"));
    }
    insta::assert_snapshot!("command_description_responsive", snapshots.join("\n\n"));
}
