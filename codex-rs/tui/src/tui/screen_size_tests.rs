use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::custom_terminal::Terminal as CustomTerminal;
use crate::test_backend::VT100Backend;
use crate::tui::TuiEvent;
use crate::tui::scrollback::ScrollbackStrategy;

#[tokio::test]
async fn draw_size_policy_refreshes_only_after_resume() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let cached = Size::new(/*width*/ 120, /*height*/ 40);
    let resumed = tui.terminal.size().expect("backend size");
    tui.terminal.last_known_screen_size = cached;
    let resized = Size::new(/*width*/ 100, /*height*/ 30);
    for (event, expected) in [
        (TuiEvent::Draw, cached),
        (TuiEvent::Resume, resumed),
        (TuiEvent::Resize(resized), resized),
        (TuiEvent::Paste(String::new()), cached),
        (TuiEvent::Draw, resized),
        (TuiEvent::Paste(String::new()), cached),
    ] {
        assert_eq!(tui.screen_size_for_event(&event).expect("size"), expected);
        if matches!(event, TuiEvent::Resize(_)) {
            tui.defer_screen_size(resized);
        }
    }
    assert_eq!(tui.take_event_screen_size().expect("size"), resumed);
    assert!(tui.screen_size.pending_recheck_at.is_none());
}

#[tokio::test]
async fn standalone_resize_draw_rechecks_settled_screen_size_once() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let resize_size = Size::new(/*width*/ 120, /*height*/ 40);

    tui.screen_size_for_event(&TuiEvent::Resize(resize_size))
        .expect("resolve resize");
    tui.terminal.resize(resize_size).expect("apply resize");
    assert_eq!(
        tui.screen_size_for_event(&TuiEvent::Draw)
            .expect("resolve early draw"),
        resize_size
    );

    tui.schedule_screen_size_recheck(Duration::ZERO);
    assert_eq!(
        tui.screen_size_for_event(&TuiEvent::Draw)
            .expect("resolve settled draw"),
        tui.terminal.size().expect("terminal size")
    );
    assert!(tui.screen_size.pending_recheck_at.is_none());
}

#[tokio::test]
async fn entering_alternate_screen_updates_cached_screen_size() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let screen_size = tui.terminal.size().expect("terminal size");
    tui.terminal.last_known_screen_size = Size::new(/*width*/ 120, /*height*/ 40);
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 8,
        screen_size.width,
        screen_size.height.saturating_sub(8),
    ));
    tui.terminal.note_history_rows_inserted(/*inserted_rows*/ 2);
    let inline_state = tui.terminal.inline_viewport_state();

    tui.enter_alt_screen().expect("enter alternate screen");

    assert_eq!(tui.terminal.last_known_screen_size, screen_size);
    assert_eq!(tui.terminal.visible_history_rows(), 0);
    tui.leave_alt_screen().expect("leave alternate screen");
    assert_eq!(tui.terminal.inline_viewport_state(), inline_state);
}

#[tokio::test]
async fn inline_viewport_starts_bottom_aligned_and_stays_docked_when_content_shrinks() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let screen_size = Size::new(/*width*/ 80, /*height*/ 24);
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 0,
    ));
    tui.terminal.last_known_screen_size = screen_size;
    let scrollback = tui.scrollback;

    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut tui.terminal,
        /*height*/ 14,
        screen_size,
        scrollback,
    )
    .expect("dock initial viewport");
    assert_eq!(
        tui.terminal.viewport_area,
        Rect::new(
            /*x*/ 0, /*y*/ 10, /*width*/ 80, /*height*/ 14
        )
    );

    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut tui.terminal,
        /*height*/ 8,
        screen_size,
        scrollback,
    )
    .expect("keep shrunken viewport docked");
    assert_eq!(
        tui.terminal.viewport_area,
        Rect::new(
            /*x*/ 0, /*y*/ 16, /*width*/ 80, /*height*/ 8
        )
    );
}

#[test]
fn full_screen_provider_popup_close_repaints_shorter_composer_viewport() {
    let screen_size = Size::new(/*width*/ 32, /*height*/ 12);
    let backend = VT100Backend::with_scrollback(
        screen_size.width,
        screen_size.height,
        /*scrollback_len*/ 32,
    );
    let mut terminal = CustomTerminal::with_options(backend).expect("terminal");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 4,
        screen_size.width,
        /*height*/ 8,
    ));
    for (row, text) in [
        (0, "shell-history-marker"),
        (2, "history-tail-one"),
        (3, "history-tail-two"),
        (4, "stale-provider-title"),
        (5, "stale-provider-row-one"),
        (6, "stale-provider-row-two"),
        (7, "stale-provider-row-three"),
        (8, "stale-provider-row-four"),
        (9, "stale-provider-row-five"),
        (10, "stale-provider-footer"),
        (11, "stale-provider-hint"),
    ] {
        queue!(terminal.backend_mut(), MoveTo(/*x*/ 0, row), Print(text))
            .expect("seed terminal row");
    }
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    let needs_full_repaint = crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 4,
        screen_size,
        ScrollbackStrategy::FullScreen,
    )
    .expect("shrink provider popup viewport");
    assert!(needs_full_repaint);
    assert_eq!(
        terminal.viewport_area,
        Rect::new(
            /*x*/ 0,
            /*y*/ 8,
            screen_size.width,
            /*height*/ 4,
        )
    );

    terminal.invalidate_viewport();
    let composer_area = terminal.viewport_area;
    terminal
        .draw_with_size(screen_size, |frame| {
            Paragraph::new(vec![
                Line::from("composer-top"),
                Line::from("│› ready"),
                Line::from("────────────────"),
                Line::from("  ? shortcuts"),
            ])
            .render(composer_area, frame.buffer_mut());
        })
        .expect("draw shorter composer viewport");

    let contents = terminal.backend().vt100().screen().contents();
    assert!(!contents.contains("stale-provider"), "{contents}");
    insta::assert_snapshot!(contents, @r"
    shell-history-marker





    history-tail-one
    history-tail-two
    composer-top
    │› ready
    ────────────────
      ? shortcuts
    ");
}

#[test]
fn terminal_width_change_leaves_tracked_rows_for_source_reflow() {
    let screen_size = Size::new(/*width*/ 24, /*height*/ 8);
    let backend = VT100Backend::new(screen_size.width, screen_size.height);
    let mut terminal = CustomTerminal::with_options(backend).expect("terminal");
    terminal.last_known_screen_size = Size::new(/*width*/ 20, /*height*/ 8);
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 4, /*width*/ 20, /*height*/ 4,
    ));
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, /*y*/ 2),
        Print("shell-marker"),
        MoveTo(/*x*/ 0, /*y*/ 3),
        Print("source-reflow-row")
    )
    .expect("seed physical rows");

    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 2,
        screen_size,
        ScrollbackStrategy::FullScreen,
    )
    .expect("resize inline viewport");

    assert_eq!(
        (terminal.viewport_area, terminal.visible_history_rows()),
        (Rect::new(0, 6, 24, 2), 2)
    );
    insta::assert_snapshot!(terminal.backend().vt100().screen().contents());
}

#[test]
fn terminal_height_shrink_then_growth_leaves_rows_for_source_reflow() {
    let full_size = Size::new(/*width*/ 24, /*height*/ 8);
    let shrunken_size = Size::new(/*width*/ 24, /*height*/ 6);
    let backend = VT100Backend::new(full_size.width, full_size.height);
    let mut terminal = CustomTerminal::with_options(backend).expect("terminal");
    terminal.last_known_screen_size = full_size;
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 4, /*width*/ 24, /*height*/ 4,
    ));
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 2,
        shrunken_size,
        ScrollbackStrategy::FullScreen,
    )
    .expect("shrink inline viewport");
    terminal
        .resize(shrunken_size)
        .expect("record shrunken size");
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, /*y*/ 2),
        Print("shell-row-one"),
        MoveTo(/*x*/ 0, /*y*/ 3),
        Print("shell-row-two")
    )
    .expect("seed rows written during resize debounce");

    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 2,
        full_size,
        ScrollbackStrategy::FullScreen,
    )
    .expect("grow inline viewport");

    assert_eq!(
        (terminal.viewport_area, terminal.visible_history_rows()),
        (Rect::new(0, 6, 24, 2), 2)
    );
    insta::assert_snapshot!(terminal.backend().vt100().screen().contents());
}
