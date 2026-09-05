use super::*;
use pretty_assertions::assert_eq;

#[test]
fn selected_description_stays_readable_and_navigation_keeps_a_stable_height() {
    let mut snapshots = Vec::new();
    for width in [24, 48, 80, 120] {
        let mut popup = CommandPopup::new(CommandPopupFlags::default(), Vec::new());
        let height = popup.calculate_required_height(width);
        popup.move_down();
        assert_eq!(popup.calculate_required_height(width), height);
        popup.on_composer_text_change("/ide".to_string());
        let height = popup.calculate_required_height(width);
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buffer = Buffer::empty(area);
        popup.render_ref(area, &mut buffer);
        let text = (0..height).map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("/ide"), "{text}");
        if width >= 48 {
            let visible_description = text.lines().skip(2).map(str::trim).collect::<Vec<_>>().join(" ");
            assert!(visible_description.contains(popup.selected_item().expect("selected command").description()), "{text}");
        }
        snapshots.push(format!("{width} columns\n{buffer:?}"));
    }
    insta::assert_snapshot!("command_description_responsive", snapshots.join("\n\n"));
}
