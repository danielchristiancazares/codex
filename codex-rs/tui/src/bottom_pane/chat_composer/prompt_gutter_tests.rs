use pretty_assertions::assert_eq;
use ratatui::style::Modifier;

use super::PromptGutterState;

fn state() -> PromptGutterState {
    PromptGutterState {
        input_enabled: true,
        is_bash_mode: false,
        effort_tier: None,
        ignition_charge: 1.0,
        selected_remote_image_index: None,
    }
}

#[test]
fn idle_composer_shows_the_caret_on_the_text_area_row() {
    let gutter = state().resolve();

    assert_eq!(
        gutter
            .textarea
            .as_ref()
            .map(|span| span.content.as_ref().to_string()),
        Some("›".to_string())
    );
    assert_eq!(gutter.remote_image_row, None);
}

#[test]
fn shell_mode_replaces_the_caret_glyph() {
    let gutter = PromptGutterState {
        is_bash_mode: true,
        ..state()
    }
    .resolve();

    assert_eq!(
        gutter
            .textarea
            .as_ref()
            .map(|span| span.content.as_ref().to_string()),
        Some("!".to_string())
    );
}

#[test]
fn selected_remote_image_row_takes_sole_ownership_of_the_caret() {
    let gutter = PromptGutterState {
        selected_remote_image_index: Some(1),
        ..state()
    }
    .resolve();

    assert_eq!(gutter.textarea, None);
    assert_eq!(
        gutter
            .remote_image_row
            .as_ref()
            .map(|(index, span)| (*index, span.content.as_ref().to_string())),
        Some((1, "›".to_string()))
    );
}

#[test]
fn disabled_input_dims_the_caret_and_never_moves_it() {
    let gutter = PromptGutterState {
        input_enabled: false,
        selected_remote_image_index: Some(0),
        ..state()
    }
    .resolve();

    let caret = gutter.textarea.expect("disabled composer keeps a caret");
    assert!(caret.style.add_modifier.contains(Modifier::DIM));
    assert_eq!(gutter.remote_image_row, None);
}
