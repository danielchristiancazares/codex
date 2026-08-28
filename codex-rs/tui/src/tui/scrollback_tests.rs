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

    ScrollbackStrategy::FullScreen
        .grow_viewport(
            &mut terminal,
            viewport_top,
            Size::new(width, height),
            /*scroll_by*/ 2,
        )
        .expect("grow viewport through tracked gap");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ previous_viewport_top,
        width,
        height - previous_viewport_top,
    ));
    terminal
        .clear_after_position(Position::new(/*x*/ 0, previous_viewport_top))
        .expect("clear expanded viewport");

    assert_eq!(
        (
            terminal.visible_history_rows(),
            terminal.docked_history_gap_rows(),
        ),
        (2, 0)
    );
    let visible = terminal.backend().vt100().screen().contents();
    let mut scrollback_screen = terminal.backend().vt100().screen().clone();
    scrollback_screen.set_scrollback(/*rows*/ usize::MAX);
    assert_eq!(scrollback_screen.contents(), visible);
    insta::assert_snapshot!(visible, @r"

    shell-history-marker




    history-tail-one
    history-tail-two
    ");
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

#[test]
fn full_screen_inserts_consume_docked_blank_band_before_scrolling() {
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
        (5, "history-before-tail"),
        (6, "history-tail-one"),
        (7, "history-tail-two"),
    ] {
        queue!(terminal.backend_mut(), MoveTo(/*x*/ 0, row), Print(line))
            .expect("seed terminal row");
    }
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    ScrollbackStrategy::FullScreen
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

    let mut insertion_states = Vec::new();
    for (line, expected_gap_rows) in [
        ("new-history-one", 1),
        ("new-history-two", 0),
        ("new-history-three", 0),
    ] {
        insert_history_lines_with_mode_and_wrap_policy(
            &mut terminal,
            vec![Line::from(line)],
            InsertHistoryMode::FullScreen,
            HistoryLineWrapPolicy::PreWrap,
        )
        .expect("insert pending history");
        assert_eq!(
            (
                terminal.viewport_area.top(),
                terminal.docked_history_gap_rows(),
            ),
            (10, expected_gap_rows)
        );
        let viewport = terminal.viewport_area;
        assert_eq!(viewport.bottom(), height);
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, viewport.top()),
            Print("composer-top"),
            MoveTo(/*x*/ 0, viewport.top() + 1),
            Print("composer-bottom")
        )
        .expect("redraw bottom-docked composer");
        let rows: Vec<String> = terminal
            .backend()
            .vt100()
            .screen()
            .rows(/*start*/ 0, width)
            .collect();
        assert!(
            rows[..viewport.top() as usize]
                .iter()
                .any(|row| row.contains(line)),
            "inserted history should remain above the viewport: {rows:?}"
        );
        assert!(
            rows[viewport.top() as usize..]
                .iter()
                .all(|row| !row.contains("history")),
            "history must not overlap the live viewport: {rows:?}"
        );
        assert_eq!(
            rows[viewport.top() as usize..]
                .iter()
                .map(|row| row.trim_end())
                .collect::<Vec<_>>(),
            ["composer-top", "composer-bottom"]
        );
        insertion_states.push(format!(
            "{line} (gap {expected_gap_rows}):\n{}",
            terminal.backend().vt100().screen().contents()
        ));
    }
    insta::assert_snapshot!(insertion_states.join("\n---\n"), @r"
    new-history-one (gap 1):





    history-before-tail

    history-tail-one
    history-tail-two
    new-history-one
    composer-top
    composer-bottom
    ---
    new-history-two (gap 0):





    history-before-tail
    history-tail-one
    history-tail-two
    new-history-one
    new-history-two
    composer-top
    composer-bottom
    ---
    new-history-three (gap 0):




    history-before-tail
    history-tail-one
    history-tail-two
    new-history-one
    new-history-two
    new-history-three
    composer-top
    composer-bottom
    ");

    let rows: Vec<String> = terminal
        .backend()
        .vt100()
        .screen()
        .rows(/*start*/ 0, width)
        .collect();
    let positions = [
        "history-before-tail",
        "history-tail-one",
        "history-tail-two",
        "new-history-one",
        "new-history-two",
        "new-history-three",
    ]
    .map(|needle| {
        rows.iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("missing {needle:?} in {rows:?}"))
    });

    assert_eq!(positions, [4, 5, 6, 7, 8, 9], "history rows: {rows:?}");
    let contents = rows
        .iter()
        .map(|row| row.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(contents, @r"




history-before-tail
history-tail-one
history-tail-two
new-history-one
new-history-two
new-history-three

    ");
}

#[test]
fn full_screen_wrapped_inserts_consume_gap_by_physical_rows() {
    let width = 12;
    let height = 8;
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 16);
    let mut terminal = Terminal::with_options(backend).expect("terminal with scrollback");
    let previous_viewport_top = 3;
    let viewport_top = 6;
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        previous_viewport_top,
        width,
        height - previous_viewport_top,
    ));
    for (row, line) in [(1, "tail-one"), (2, "tail-two")] {
        queue!(terminal.backend_mut(), MoveTo(/*x*/ 0, row), Print(line))
            .expect("seed terminal row");
    }
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);

    ScrollbackStrategy::FullScreen
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

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("alpha beta gamma")],
        InsertHistoryMode::FullScreen,
        HistoryLineWrapPolicy::PreWrap,
    )
    .expect("insert two pre-wrapped rows");
    assert_eq!(
        (
            terminal.viewport_area,
            terminal.visible_history_rows(),
            terminal.docked_history_gap_rows(),
        ),
        (Rect::new(0, 6, width, 2), 4, 1)
    );

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("abcdefghijklmnop")],
        InsertHistoryMode::FullScreen,
        HistoryLineWrapPolicy::Terminal,
    )
    .expect("insert two terminal-wrapped rows");
    assert_eq!(
        (
            terminal.viewport_area,
            terminal.visible_history_rows(),
            terminal.docked_history_gap_rows(),
        ),
        (Rect::new(0, 6, width, 2), 6, 0)
    );

    let viewport = terminal.viewport_area;
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, viewport.top()),
        Print("composer"),
        MoveTo(/*x*/ 0, viewport.top() + 1),
        Print("footer")
    )
    .expect("draw bottom-docked viewport");
    let rows: Vec<String> = terminal
        .backend()
        .vt100()
        .screen()
        .rows(/*start*/ 0, width)
        .collect();
    assert_eq!(
        rows[viewport.top() as usize..]
            .iter()
            .map(|row| row.trim_end())
            .collect::<Vec<_>>(),
        ["composer", "footer"]
    );
    insta::assert_snapshot!(
        rows.iter()
            .map(|row| row.trim_end())
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    tail-one
    tail-two
    alpha beta
    gamma
    abcdefghijkl
    mnop
    composer
    footer"
    );
}
