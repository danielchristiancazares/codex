use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn type_text(page: &mut NewTaskPage, text: &str) {
    for character in text.chars() {
        assert!(matches!(
            page.input(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE,)),
            NewTaskInput::None
        ));
        std::thread::sleep(ComposerInput::recommended_flush_delay());
        page.composer.flush_paste_burst_if_due();
        if character == ' ' {
            let _ = page.input(KeyEvent::new_with_kind(
                KeyCode::Char(' '),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ));
        }
    }
}

#[test]
fn slash_shaped_prompt_reaches_task_creation_once() {
    let mut page = NewTaskPage::new(Some("env-1".to_string()), /*best_of_n*/ 2);
    type_text(&mut page, "/diff");
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let mut submissions = Vec::new();

    for _ in 0..2 {
        if let NewTaskInput::Submitted(submission) = page.input(enter) {
            submissions.push(submission);
        }
    }

    assert_eq!(submissions.len(), 1);
    let submission = submissions.pop().expect("submission");
    assert_eq!(submission.env_id, "env-1");
    assert_eq!(submission.prompt, "/diff");
    assert_eq!(submission.best_of_n, 2);
}
