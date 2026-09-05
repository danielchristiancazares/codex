use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn landing_preserves_draft_and_cursor_across_resizes_then_yields_to_activity() {
    let (mut chat, _tx, _rx, _ops) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    chat.thread_id = Some(ThreadId::new());
    let text = "Review the changes in 日本語\nKeep this draft intact.";
    chat.handle_paste(text.to_string());
    let mut frames = Vec::new();
    for (width, height) in [(120, 36), (48, 20), (18, 12), (80, 24), (24, 4)] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let surface = chat.landing_surface().expect("idle session landing");
        let mut buffer = Buffer::empty(area);
        surface.render(area, &mut buffer);
        let cursor = surface.cursor_pos(area).expect("editable draft cursor");
        assert!(
            area.contains(cursor.into()),
            "cursor {cursor:?} outside {area:?}"
        );
        frames.push(format!("{width}x{height}, cursor {cursor:?}\n{buffer:?}"));
        assert_eq!(chat.composer_text_with_pending(), text);
    }
    insta::assert_snapshot!("landing_responsive_draft_cursor", frames.join("\n\n"));
    chat.input_queue.user_turn_pending_start = true;
    assert!(chat.landing_surface().is_none());
    assert_eq!(chat.composer_text_with_pending(), text);
    chat.input_queue.user_turn_pending_start = false;
    chat.transcript.active_cell = Some(Box::new(history_cell::new_warning_event(
        "Check the workspace".to_string(),
    )));
    assert!(chat.landing_surface().is_none());
}
