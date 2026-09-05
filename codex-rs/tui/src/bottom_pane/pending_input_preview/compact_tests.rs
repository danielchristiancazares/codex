use super::*;
use pretty_assertions::assert_eq;

#[test]
fn compact_preview_keeps_the_edit_target_and_accounts_for_every_older_input() {
    let mut preview = PendingInputPreview::new();
    preview.queued_messages = (1..=10).map(|index| format!("Follow-up {index}")).collect();
    let queued = preview.queued_messages.clone();
    for width in [24, 40, 48, 80] {
        let lines = preview.preview_lines(width, VISIBLE_ROW_CAP, QuestionPresence::Absent);
        assert!(lines.len() <= if width < 60 { 3 } else { 5 });
        let text = lines.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Latest: Follow-up 10"), "{text}");
        let visible = text.matches("Follow-up").count();
        assert!(text.contains(&format!("… {} older queued", queued.len() - visible)), "{text}");
        assert!(text.contains("edit queue"), "{text}");
    }
    assert_eq!(preview.queued_messages, queued);
    preview.rejected_steers.push("Retry first".to_string());
    preview.pending_steers.push("Steer second".to_string());
    let lines = preview.preview_lines(/*width*/ 48, VISIBLE_ROW_CAP, QuestionPresence::Absent);
    let text = lines.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
    let retry = text.find("Retry: Retry first").expect("required retry");
    let steer = text.find("Steer: Steer second").expect("pending steer");
    let latest = text.find("Latest: Follow-up 10").expect("editable queued input");
    assert!(retry < steer && steer < latest, "{text}");
    insta::assert_snapshot!("compact_queue_priority", text);
}
