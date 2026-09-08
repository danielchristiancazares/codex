use super::*;
use crate::terminal_palette::with_test_terminal_palette;
use crate::terminal_probe::DefaultColors;
use crate::test_support::export_visual_review_buffer;
use pretty_assertions::assert_eq;

#[test]
fn composer_focus_visual_review_gallery() {
    enum ComposerState {
        Focused,
        Unfocused,
        ParentOwned,
        Unavailable,
    }
    for (palette, fg, bg, level) in [
        (
            "dark",
            (220, 220, 216),
            (32, 32, 32),
            StdoutColorLevel::TrueColor,
        ),
        (
            "light",
            (36, 36, 36),
            (250, 249, 246),
            StdoutColorLevel::TrueColor,
        ),
        (
            "ansi16",
            (220, 220, 216),
            (32, 32, 32),
            StdoutColorLevel::Ansi16,
        ),
    ] {
        for (label, state) in [
            ("focused", ComposerState::Focused),
            ("unfocused", ComposerState::Unfocused),
            ("disabled", ComposerState::ParentOwned),
            ("unavailable", ComposerState::Unavailable),
        ] {
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            let mut composer = ChatComposer::new(
                !matches!(state, ComposerState::Unfocused),
                AppEventSender::new(tx),
                /*enhanced_keys_supported*/ false,
                "Ask Codex to do anything".into(),
                /*disable_paste_burst*/ true,
            );
            composer.handle_paste("Review the retry fix.".to_string());
            match state {
                ComposerState::ParentOwned => composer.set_parent_owned_thread(),
                ComposerState::Unavailable => {
                    composer.set_input_enabled(/*enabled*/ false, /*placeholder*/ None)
                }
                ComposerState::Focused | ComposerState::Unfocused => {}
            }
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 6,
            );
            let mut buffer = Buffer::empty(area);
            with_test_terminal_palette(DefaultColors { fg, bg }, level, || {
                crate::key_hint::with_test_native_key_labels(|| composer.render(area, &mut buffer));
                assert_eq!(
                    composer.cursor_pos(area).is_some(),
                    matches!(state, ComposerState::Focused)
                );
                if let Some((x, y)) = composer.cursor_pos(area) {
                    buffer[(x, y)].set_style(Style::default().reversed());
                }
                export_visual_review_buffer(&format!("composer_{label}_{palette}"), &buffer);
            });
            insta::assert_snapshot!(
                format!("composer_{label}_{palette}").as_str(),
                format!("{buffer:?}")
            );
        }
    }
}

#[test]
fn composer_popup_hint_preserves_enter_selection_and_configured_submission() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex to do anything".into(),
        /*disable_paste_burst*/ true,
    );
    composer.submit_keys = vec![key_hint::plain(KeyCode::F(6))];
    composer.handle_paste("/model".to_string());
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 80,
        composer.desired_height(/*width*/ 80),
    );
    let mut buffer = Buffer::empty(area);
    composer.render(area, &mut buffer);
    let frame = format!("{buffer:?}");
    assert!(frame.contains("enter select"), "{frame}");
    assert!(
        frame.contains("navigate") && frame.contains("esc close"),
        "{frame}"
    );
    assert_eq!(
        composer
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .0,
        InputResult::Command(SlashCommand::Model)
    );
    composer.handle_paste("Review the retry fix.".to_string());
    assert!(matches!(
        composer
            .handle_key_event(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE))
            .0,
        InputResult::Submitted { .. }
    ));
}
