use super::*;
use crate::key_hint;
use crate::keymap::RuntimeKeymap;
use pretty_assertions::assert_eq;

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn wide_generated_footer_keeps_verbose_wording() {
    let keymap = RuntimeKeymap::defaults().list;
    let footer = MultiSelectFooter::new(Vec::new(), &keymap, /*ordering_enabled*/ true);

    let line = footer.line_for_width(/*width*/ 100).expect("footer");

    assert_eq!(
        line_text(line),
        "Press space to toggle; ←/→ to move; enter to confirm and close; esc to close"
    );
}

#[test]
fn status_line_width_uses_complete_compact_actions() {
    let keymap = RuntimeKeymap::defaults().list;
    let footer = MultiSelectFooter::new(Vec::new(), &keymap, /*ordering_enabled*/ true);

    let line = footer.line_for_width(/*width*/ 70).expect("compact footer");

    assert_eq!(
        line_text(line),
        "enter confirm; esc close; space toggle; ←/→ move"
    );
}

#[test]
fn compact_footer_uses_remapped_accept_and_cancel_keys() {
    let mut keymap = RuntimeKeymap::defaults().list;
    keymap.accept = vec![key_hint::plain(KeyCode::F(12))];
    keymap.cancel = vec![key_hint::ctrl(KeyCode::Char('q'))];
    let footer = MultiSelectFooter::new(Vec::new(), &keymap, /*ordering_enabled*/ true);

    let line = footer.line_for_width(/*width*/ 60).expect("compact footer");
    let text = line_text(line);

    assert!(text.contains("f12 confirm"), "{text:?}");
    assert!(text.contains("ctrl + q close"), "{text:?}");
}

#[test]
fn disabled_cancel_binding_is_omitted() {
    let mut keymap = RuntimeKeymap::defaults().list;
    keymap.cancel.clear();
    let footer = MultiSelectFooter::new(Vec::new(), &keymap, /*ordering_enabled*/ false);

    let line = footer.line_for_width(/*width*/ 30).expect("compact footer");
    let text = line_text(line);

    assert!(text.contains("enter confirm"), "{text:?}");
    assert!(!text.contains("esc"), "{text:?}");
    assert!(!text.contains("close"), "{text:?}");
}

#[test]
fn generated_footer_omits_text_when_no_complete_candidate_fits() {
    let keymap = RuntimeKeymap::defaults().list;
    let footer = MultiSelectFooter::new(Vec::new(), &keymap, /*ordering_enabled*/ true);

    assert_eq!(footer.line_for_width(/*width*/ 2), None);
}

#[test]
fn custom_footer_is_not_replaced_at_narrow_widths() {
    let keymap = RuntimeKeymap::defaults().list;
    let footer = MultiSelectFooter::new(
        vec!["Custom instructions".into()],
        &keymap,
        /*ordering_enabled*/ true,
    );

    let line = footer.line_for_width(/*width*/ 1).expect("custom footer");

    assert_eq!(line_text(line), "Custom instructions");
}
