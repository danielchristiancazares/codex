use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;

fn type_text(composer: &mut ComposerInput, text: &str) {
    for character in text.chars() {
        assert!(matches!(
            composer.input(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE,)),
            ComposerAction::None
        ));
        std::thread::sleep(ComposerInput::recommended_flush_delay());
        composer.flush_paste_burst_if_due();
        if character == ' ' {
            let _ = composer.input(KeyEvent::new_with_kind(
                KeyCode::Char(' '),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ));
        }
    }
}

fn submit(composer: &mut ComposerInput) -> String {
    match composer.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
        ComposerAction::Submitted(text) => text,
        ComposerAction::None => panic!("expected composer submission"),
    }
}

#[test]
fn slash_commands_submit_as_literal_text() {
    for prompt in ["/diff", "/model", "/review custom text", "@file"] {
        let mut composer = ComposerInput::new();
        type_text(&mut composer, prompt);

        assert_eq!(submit(&mut composer), prompt);
    }
}

#[test]
fn pasted_image_looking_path_submits_as_literal_text() {
    let mut composer = ComposerInput::new();
    let path = r"C:\workspace\screenshot.png";

    assert!(composer.handle_paste(path.to_string()));

    assert_eq!(submit(&mut composer), path);
}

#[test]
fn slash_prompt_renders_as_plain_text() {
    let mut composer = ComposerInput::new();
    type_text(&mut composer, "/diff");
    let width = 40;
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        composer.desired_height(width),
    );
    let mut buffer = Buffer::empty(area);

    composer.render_ref(area, &mut buffer);

    let rendered = (0..area.height)
        .map(|row| {
            (0..area.width)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_snapshot!("composer_input_plain_text_slash_prompt", rendered);
}
