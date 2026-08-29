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
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
use crate::insert_history::insert_history_lines_with_mode_and_wrap_policy;
use crate::test_backend::VT100Backend;
use crate::tui::InlineViewportPlacement;
use crate::tui::InlineViewportRole;
use crate::tui::TuiEvent;
use crate::tui::scrollback::HistoryTailDock;
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
    tui.terminal.note_docked_history_gap(/*rows*/ 2);
    let inline_state = tui.terminal.inline_viewport_state();

    tui.enter_alt_screen().expect("enter alternate screen");

    assert_eq!(tui.terminal.last_known_screen_size, screen_size);
    assert_eq!(
        (
            tui.terminal.visible_history_rows(),
            tui.terminal.docked_history_gap_rows(),
        ),
        (0, 0)
    );
    tui.leave_alt_screen().expect("leave alternate screen");
    assert_eq!(tui.terminal.inline_viewport_state(), inline_state);
}

#[tokio::test]
async fn alternate_screen_resize_clears_saved_inline_history_tracking() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let screen_size = tui.terminal.size().expect("terminal size");
    tui.terminal.last_known_screen_size = screen_size;
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 8,
        screen_size.width,
        screen_size.height.saturating_sub(/*rhs*/ 8),
    ));
    tui.terminal.note_history_rows_inserted(/*inserted_rows*/ 2);
    tui.terminal.note_docked_history_gap(/*rows*/ 2);
    tui.enter_alt_screen().expect("enter alternate screen");

    let resized_size = Size::new(
        screen_size.width.saturating_add(/*rhs*/ 4),
        screen_size.height,
    );
    tui.screen_size_for_event(&TuiEvent::Resize(resized_size))
        .expect("resolve alternate-screen resize");
    tui.draw(u16::MAX, |_| {})
        .expect("draw resized alternate screen");
    tui.leave_alt_screen().expect("leave alternate screen");

    assert_eq!(
        (
            tui.terminal.visible_history_rows(),
            tui.terminal.docked_history_gap_rows(),
        ),
        (0, 0)
    );
}

#[tokio::test]
async fn alternate_screen_height_resize_restores_bottom_docked_inline_viewport() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("test tui");
    let initial_size = Size::new(/*width*/ 80, /*height*/ 24);
    let resized_size = Size::new(/*width*/ 80, /*height*/ 30);
    tui.terminal.last_known_screen_size = initial_size;
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 19,
        initial_size.width,
        /*height*/ 5,
    ));
    tui.enter_alt_screen().expect("enter alternate screen");

    tui.screen_size_for_event(&TuiEvent::Resize(resized_size))
        .expect("resolve alternate-screen resize");
    tui.draw(u16::MAX, |_| {})
        .expect("draw resized alternate screen");
    tui.leave_alt_screen().expect("leave alternate screen");
    tui.draw_with_resize_reflow(
        /*height*/ 5,
        resized_size,
        InlineViewportPlacement::FollowExisting,
        InlineViewportRole::Persistent,
        |_| {},
    )
    .expect("draw restored inline viewport");

    assert_eq!(
        tui.terminal.viewport_area,
        Rect::new(
            /*x*/ 0,
            /*y*/ 25,
            resized_size.width,
            /*height*/ 5,
        )
    );
    insta::assert_debug_snapshot!(
        (resized_size, tui.terminal.viewport_area),
        @r"
    (
        Size {
            width: 80,
            height: 30,
        },
        Rect {
            x: 0,
            y: 25,
            width: 80,
            height: 5,
        },
    )
    "
    );
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
        InlineViewportPlacement::FollowExisting,
        scrollback,
        HistoryTailDock::Immediate,
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
        InlineViewportPlacement::FollowExisting,
        scrollback,
        HistoryTailDock::Immediate,
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
fn full_screen_popup_close_preserves_history_position() {
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
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::PreservePosition,
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

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("new-history-row")],
        InsertHistoryMode::FullScreen,
        HistoryLineWrapPolicy::PreWrap,
    )
    .expect("insert history after popup close");
    assert_eq!(
        (
            terminal.viewport_area,
            terminal.visible_history_rows(),
            terminal.docked_history_gap_rows(),
        ),
        (Rect::new(0, 8, screen_size.width, 4), 3, 0)
    );
    assert_eq!(terminal.viewport_area.bottom(), screen_size.height);

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
        .expect("redraw composer after history insert");
    let needs_full_repaint = crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 4,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("repeat same-size viewport update");
    assert!(!needs_full_repaint);
    assert_eq!(
        terminal.viewport_area,
        Rect::new(0, 8, screen_size.width, 4)
    );

    let contents = terminal.backend().vt100().screen().contents();
    insta::assert_snapshot!(contents, @r"
    history-tail-one
    history-tail-two




    new-history-row
    composer-top
    │› ready
    ────────────────
      ? shortcuts
    ");
}

#[test]
fn full_screen_popup_regrow_discards_only_reclaimed_gap_rows() {
    let screen_size = Size::new(/*width*/ 24, /*height*/ 12);
    let backend = VT100Backend::with_scrollback(
        screen_size.width,
        screen_size.height,
        /*scrollback_len*/ 32,
    );
    let mut terminal = CustomTerminal::with_options(backend).expect("terminal with scrollback");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 8,
        screen_size.width,
        /*height*/ 4,
    ));
    for row in 0..8u16 {
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, row),
            Print(format!("history-row-{row}"))
        )
        .expect("seed terminal row");
    }
    terminal.note_history_rows_inserted(/*inserted_rows*/ 8);

    // A popup opens: the viewport grows and the screen scrolls up.
    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 8,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("open popup viewport");
    assert_eq!(
        terminal.viewport_area,
        Rect::new(
            /*x*/ 0,
            /*y*/ 4,
            screen_size.width,
            /*height*/ 8
        )
    );

    // Filtering shrinks the popup: the tail docks and leaves a tracked gap.
    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 5,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("shrink popup viewport");
    assert_eq!(
        (terminal.viewport_area, terminal.docked_history_gap_rows()),
        (
            Rect::new(
                /*x*/ 0,
                /*y*/ 7,
                screen_size.width,
                /*height*/ 5
            ),
            3
        )
    );

    // Typing regrows the popup by two rows: only two gap rows are reclaimed, so the
    // remaining gap row must stay tracked above a still-adjacent history tail.
    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 7,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("regrow popup viewport");

    assert_eq!(
        (
            terminal.viewport_area,
            terminal.visible_history_rows(),
            terminal.docked_history_gap_rows(),
        ),
        (
            Rect::new(
                /*x*/ 0,
                /*y*/ 5,
                screen_size.width,
                /*height*/ 7
            ),
            4,
            1
        )
    );
    let rows: Vec<String> = terminal
        .backend()
        .vt100()
        .screen()
        .rows(/*start*/ 0, screen_size.width)
        .collect();
    assert_eq!(
        rows[1..5]
            .iter()
            .map(|row| row.trim_end())
            .collect::<Vec<_>>(),
        [
            "history-row-4",
            "history-row-5",
            "history-row-6",
            "history-row-7",
        ],
        "history tail must stay adjacent to the regrown viewport: {rows:?}"
    );
}

#[test]
fn full_height_viewport_collapse_keeps_flushed_history_adjacent() {
    let width = 32;
    let height = 12;
    let screen_size = Size::new(width, height);
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = CustomTerminal::with_options(backend).expect("terminal with scrollback");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 8, width, /*height*/ 4,
    ));
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, /*y*/ 7),
        Print("prior-commentary")
    )
    .expect("seed history tail");
    terminal.note_history_rows_inserted(/*inserted_rows*/ 1);

    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ height,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("expand live viewport to full height");
    assert_eq!(terminal.viewport_area, Rect::new(0, 0, width, height));

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("Ran 2 commands")],
        InsertHistoryMode::FullScreen,
        HistoryLineWrapPolicy::PreWrap,
    )
    .expect("flush completed command activity while expanded");

    crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 4,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("collapse live viewport");
    assert_eq!(terminal.viewport_area, Rect::new(0, 0, width, 4));

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
        .expect("draw compact live viewport");

    let visible = terminal.backend().vt100().screen().contents();
    let mut scrollback_screen = terminal.backend().vt100().screen().clone();
    scrollback_screen.set_scrollback(/*rows*/ usize::MAX);
    let rows = scrollback_screen
        .rows(/*start*/ 0, width)
        .map(|row| row.trim_end().to_string())
        .collect::<Vec<_>>();
    let [prior_commentary, completed_commands, composer] =
        ["prior-commentary", "Ran 2 commands", "composer-top"].map(|needle| {
            rows.iter()
                .position(|row| row == needle)
                .unwrap_or_else(|| panic!("missing {needle:?} in {rows:?}"))
        });
    assert_eq!(
        [completed_commands, composer],
        [prior_commentary + 1, completed_commands + 1],
        "history and live viewport must remain adjacent: {rows:?}"
    );
    insta::assert_snapshot!(visible, @r"
    composer-top
    │› ready
    ────────────────
      ? shortcuts
    "
    );
}

#[test]
fn pending_full_screen_history_refills_vacated_rows_before_bottom_dock() {
    let width = 28;
    let height = 12;
    let screen_size = Size::new(width, height);
    let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32);
    let mut terminal = CustomTerminal::with_options(backend).expect("terminal with scrollback");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 8, width, /*height*/ 4,
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

    let needs_full_repaint = crate::tui::Tui::update_inline_viewport_for_resize_reflow(
        &mut terminal,
        /*height*/ 2,
        screen_size,
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::DeferToPendingHistory,
    )
    .expect("defer bottom docking");
    assert!(needs_full_repaint);
    assert_eq!(terminal.viewport_area, Rect::new(0, 8, width, 2));

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("new-history-one"), Line::from("new-history-two")],
        InsertHistoryMode::FullScreen,
        HistoryLineWrapPolicy::PreWrap,
    )
    .expect("insert pending history");

    assert_eq!(
        (
            terminal.viewport_area,
            terminal.visible_history_rows(),
            terminal.docked_history_gap_rows(),
        ),
        (Rect::new(0, 10, width, 2), 4, 0)
    );
    insta::assert_snapshot!(terminal.backend().vt100().screen().contents(), @r"





history-before-tail
history-tail-one
history-tail-two
new-history-one
new-history-two
    ");
}

#[test]
fn terminal_width_change_clears_stale_tracking_before_history_insertion() {
    let screen_size = Size::new(/*width*/ 24, /*height*/ 8);
    let backend = VT100Backend::new(screen_size.width, screen_size.height);
    let mut terminal = CustomTerminal::with_options(backend).expect("terminal");
    terminal.last_known_screen_size = Size::new(/*width*/ 20, /*height*/ 8);
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 4, /*width*/ 20, /*height*/ 4,
    ));
    terminal.note_history_rows_inserted(/*inserted_rows*/ 2);
    terminal.note_docked_history_gap(/*rows*/ 2);
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
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("resize inline viewport");

    assert_eq!(
        (
            terminal.viewport_area,
            terminal.visible_history_rows(),
            terminal.docked_history_gap_rows(),
        ),
        (Rect::new(0, 6, 24, 2), 0, 0)
    );

    insert_history_lines_with_mode_and_wrap_policy(
        &mut terminal,
        vec![Line::from("new-history-row")],
        InsertHistoryMode::FullScreen,
        HistoryLineWrapPolicy::PreWrap,
    )
    .expect("insert history after resize");

    let contents = terminal.backend().vt100().screen().contents();
    assert!(contents.contains("shell-marker"), "{contents}");
    assert!(contents.contains("source-reflow-row"), "{contents}");
    insta::assert_snapshot!(contents);
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
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
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
        InlineViewportPlacement::FollowExisting,
        ScrollbackStrategy::FullScreen,
        HistoryTailDock::Immediate,
    )
    .expect("grow inline viewport");

    assert_eq!(
        (terminal.viewport_area, terminal.visible_history_rows()),
        (Rect::new(0, 6, 24, 2), 0)
    );
    insta::assert_snapshot!(terminal.backend().vt100().screen().contents());
}
