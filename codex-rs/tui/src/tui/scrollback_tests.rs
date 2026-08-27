use super::ScrollbackStrategy;
use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
use crate::insert_history::insert_history_lines_with_mode_and_wrap_policy;
use crate::test_backend::VT100Backend;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use pretty_assertions::assert_eq;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;

#[test]
fn windows_terminal_uses_full_screen_unless_zellij_is_active() {
    let mut terminal = TerminalInfo {
        name: TerminalName::WindowsTerminal,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    };
    let mut strategy = ScrollbackStrategy::detect(&terminal);

    assert_eq!(
        [
            strategy.history_insertion_mode(HistoryLineWrapPolicy::PreWrap),
            strategy.history_insertion_mode(HistoryLineWrapPolicy::Terminal),
        ],
        [InsertHistoryMode::FullScreen, InsertHistoryMode::FullScreen]
    );

    terminal.multiplexer = Some(Multiplexer::Zellij { version: None });
    strategy = ScrollbackStrategy::detect(&terminal);
    assert_eq!(strategy, ScrollbackStrategy::Zellij);
}

#[test]
fn zellij_only_uses_full_screen_insertion_for_terminal_wrapped_history() {
    assert_eq!(
        [
            ScrollbackStrategy::Zellij.history_insertion_mode(HistoryLineWrapPolicy::PreWrap),
            ScrollbackStrategy::Zellij.history_insertion_mode(HistoryLineWrapPolicy::Terminal),
        ],
        [InsertHistoryMode::Standard, InsertHistoryMode::FullScreen]
    );
}

#[test]
fn full_screen_history_insertion_preserves_terminal_scrollback() {
    let width = 24;
    let height = 6;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ height - 2,
        width,
        /*height*/ 2,
    ));

    for (row, line) in [
        "oldest-history-row",
        "history-row-2",
        "history-row-3",
        "history-row-4",
        "stale-composer-1",
        "stale-composer-2",
    ]
    .into_iter()
    .enumerate()
    {
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, row as u16),
            Print(line)
        )
        .expect("seed terminal row");
    }

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("new-history-row")],
        ScrollbackStrategy::FullScreen.history_insertion_mode(HistoryLineWrapPolicy::PreWrap),
        HistoryLineWrapPolicy::PreWrap,
    )
    .expect("insert history through the full screen");

    let visible = terminal.backend().vt100().screen().contents();
    let mut scrollback_screen = terminal.backend().vt100().screen().clone();
    scrollback_screen.set_scrollback(/*rows*/ usize::MAX);
    let scrollback = scrollback_screen.contents();

    insta::assert_snapshot!(format!("SCROLLBACK:\n{scrollback}\nVISIBLE:\n{visible}"), @r"
    SCROLLBACK:
    oldest-history-row
    history-row-2
    history-row-3
    history-row-4
    new-history-row
    VISIBLE:
    history-row-2
    history-row-3
    history-row-4
    new-history-row
    ");
}

#[test]
fn full_screen_viewport_growth_preserves_terminal_scrollback() {
    let width = 24;
    let height = 6;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ height - 2,
        width,
        /*height*/ 2,
    ));

    for (row, line) in [
        "oldest-history-row",
        "history-row-2",
        "history-row-3",
        "history-row-4",
        "stale-composer-1",
        "stale-composer-2",
    ]
    .into_iter()
    .enumerate()
    {
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, row as u16),
            Print(line)
        )
        .expect("seed terminal row");
    }

    ScrollbackStrategy::FullScreen
        .grow_viewport(
            &mut terminal,
            /*viewport_top*/ height - 2,
            Size::new(width, height),
            /*scroll_by*/ 2,
        )
        .expect("grow viewport through full-screen scrolling");

    let visible = terminal.backend().vt100().screen().contents();
    let mut scrollback_screen = terminal.backend().vt100().screen().clone();
    scrollback_screen.set_scrollback(/*rows*/ usize::MAX);
    let scrollback = scrollback_screen.contents();

    insta::assert_snapshot!(format!("SCROLLBACK:\n{scrollback}\nVISIBLE:\n{visible}"), @r"
    SCROLLBACK:
    oldest-history-row
    history-row-2
    history-row-3
    history-row-4
    VISIBLE:
    history-row-3
    history-row-4
    ");
}

#[test]
fn sparse_history_tail_stays_adjacent_to_bottom_docked_viewport_after_shrink() {
    let width = 28;
    let height = 12;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    let previous_viewport_top = 8;
    let viewport_top = 10;
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        previous_viewport_top,
        width,
        height - previous_viewport_top,
    ));

    for (row, line) in [
        (1, "shell-history-marker"),
        (6, "history-tail-one"),
        (7, "history-tail-two"),
        (8, "stale-live-one"),
        (9, "stale-live-two"),
        (10, "stale-live-three"),
        (11, "stale-live-four"),
    ] {
        queue!(terminal.backend_mut(), MoveTo(/*x*/ 0, row), Print(line))
            .expect("seed terminal row");
    }
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    let moved = ScrollbackStrategy::Standard
        .dock_sparse_history_tail(&mut terminal, previous_viewport_top, viewport_top)
        .expect("dock sparse history tail");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        viewport_top,
        width,
        height - viewport_top,
    ));
    terminal
        .clear_after_position(Position::new(/*x*/ 0, viewport_top))
        .expect("clear stale live viewport");
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, viewport_top),
        Print("composer-top"),
        MoveTo(/*x*/ 0, viewport_top + 1),
        Print("composer-bottom")
    )
    .expect("draw resized live viewport");

    assert_eq!((moved, terminal.visible_history_rows()), (true, 2));
    insta::assert_snapshot!(terminal.backend().vt100().screen().contents());
}

#[test]
fn full_screen_strategy_docks_history_without_moving_unrelated_rows() {
    let width = 28;
    let height = 12;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    let previous_viewport_top = 8;
    let viewport_top = 10;
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        previous_viewport_top,
        width,
        height - previous_viewport_top,
    ));

    for (row, line) in [
        (1, "shell-history-marker"),
        (6, "history-tail-one"),
        (7, "history-tail-two"),
        (8, "stale-live-one"),
        (9, "stale-live-two"),
        (10, "stale-live-three"),
        (11, "stale-live-four"),
    ] {
        queue!(terminal.backend_mut(), MoveTo(/*x*/ 0, row), Print(line))
            .expect("seed terminal row");
    }
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    let moved = ScrollbackStrategy::FullScreen
        .dock_sparse_history_tail(&mut terminal, previous_viewport_top, viewport_top)
        .expect("dock sparse history tail");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        viewport_top,
        width,
        height - viewport_top,
    ));
    terminal
        .clear_after_position(Position::new(/*x*/ 0, viewport_top))
        .expect("clear stale live viewport");
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, viewport_top),
        Print("composer-top"),
        MoveTo(/*x*/ 0, viewport_top + 1),
        Print("composer-bottom")
    )
    .expect("draw resized live viewport");

    assert_eq!((moved, terminal.visible_history_rows()), (true, 2));
    let contents = terminal.backend().vt100().screen().contents();
    assert_eq!(contents.lines().nth(1), Some("shell-history-marker"));
    insta::assert_snapshot!(contents);
}

#[test]
fn full_history_band_stays_adjacent_to_bottom_docked_viewport_after_shrink() {
    let width = 24;
    let height = 8;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 16);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    let previous_viewport_top = 4;
    let viewport_top = 6;
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        previous_viewport_top,
        width,
        height - previous_viewport_top,
    ));
    for (row, line) in [
        "history-row-one",
        "history-row-two",
        "history-row-three",
        "history-row-four",
        "stale-live-one",
        "stale-live-two",
        "stale-live-three",
        "stale-live-four",
    ]
    .into_iter()
    .enumerate()
    {
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, row as u16),
            Print(line)
        )
        .expect("seed terminal row");
    }
    terminal.note_history_rows_inserted(previous_viewport_top);

    let moved = ScrollbackStrategy::FullScreen
        .dock_sparse_history_tail(&mut terminal, previous_viewport_top, viewport_top)
        .expect("dock full history band");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        viewport_top,
        width,
        height - viewport_top,
    ));
    terminal
        .clear_after_position(Position::new(/*x*/ 0, viewport_top))
        .expect("clear stale live viewport");
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, viewport_top),
        Print("composer-top"),
        MoveTo(/*x*/ 0, viewport_top + 1),
        Print("composer-bottom")
    )
    .expect("draw resized live viewport");

    assert_eq!((moved, terminal.visible_history_rows()), (true, 4));
    insta::assert_snapshot!(terminal.backend().vt100().screen().contents());
}
