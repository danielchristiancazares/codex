use super::*;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::test_backend::VT100Backend;
use crate::tui::scrollback::ScrollbackStrategy;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

#[test]
fn picker_transitions_keep_sparse_history_contiguous() -> Result<()> {
    let screen_size = Size::new(/*width*/ 80, /*height*/ 27);
    let history = [
        ">_ Codex",
        "GPT 6 Astra max  /model to change",
        "project/codex-rs",
        "Unrestricted access  /permissions to change",
        "",
        "Tip: Start with a task.",
        "",
        "Current working directory: project/codex-rs",
    ];
    let composer = [
        "",
        "╭──────────────────╮",
        "│ › draft          │",
        "╰──────────────────╯",
        "GPT 6 Astra · Max",
    ];
    let mut snapshots = Vec::new();
    for scrollback in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let backend = VT100Backend::with_scrollback(
            screen_size.width,
            screen_size.height,
            /*scrollback_len*/ 256,
        );
        let mut terminal = Terminal::with_options(backend)?;
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0,
            /*y*/ 22,
            screen_size.width,
            /*height*/ 5,
        ));
        for (row, line) in history.iter().enumerate() {
            queue!(
                terminal.backend_mut(),
                MoveTo(/*x*/ 0, 14 + row as u16),
                Print(line)
            )?;
        }
        terminal.note_history_rows_inserted(history.len() as u16);
        let mut previous_role = InlineViewportRole::Persistent;
        for cycle in 0..3 {
            for (height, role) in [
                (18, InlineViewportRole::Transient),
                (13, InlineViewportRole::Transient),
                (16, InlineViewportRole::Transient),
                (11, InlineViewportRole::Transient),
                (5, InlineViewportRole::Persistent),
            ] {
                InlineViewportFrame {
                    height,
                    screen_size,
                    placement: InlineViewportPlacement::BottomDocked,
                    role,
                    previous_role,
                }
                .prepare(&mut terminal, &mut Vec::new(), scrollback)?;
                previous_role = role;
                terminal.draw_with_size(screen_size, |frame| {
                    let lines = match role {
                        InlineViewportRole::Transient => {
                            vec![Line::from("Model picker"); height as usize]
                        }
                        InlineViewportRole::Persistent => {
                            composer.iter().copied().map(Line::from).collect()
                        }
                    };
                    Paragraph::new(lines).render(frame.area(), frame.buffer_mut());
                })?;
                let rows = terminal
                    .backend()
                    .vt100()
                    .screen()
                    .rows(/*start*/ 0, screen_size.width)
                    .map(|row| row.trim_end().to_owned())
                    .collect::<Vec<_>>();
                let history_end = usize::from(terminal.viewport_area.top());
                assert_eq!(
                    &rows[history_end - history.len()..history_end],
                    &history,
                    "{scrollback:?}, picker cycle {cycle}, height {height}"
                );
            }
            let actual = terminal
                .backend()
                .vt100()
                .screen()
                .rows(/*start*/ 0, screen_size.width)
                .map(|row| row.trim_end().to_owned())
                .collect::<Vec<_>>();
            let expected = std::iter::repeat_n("", /*n*/ 14)
                .chain(history)
                .chain(composer)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{scrollback:?}, picker cycle {cycle}");
            if cycle == 0 {
                snapshots.push(format!("{scrollback:?}\n{}", actual.join("\n")));
            }
        }
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
    Ok(())
}

#[test]
fn queued_history_stays_adjacent_when_a_picker_closes() -> Result<()> {
    let screen_size = Size::new(/*width*/ 32, /*height*/ 12);
    for placement in [
        InlineViewportPlacement::FollowExisting,
        InlineViewportPlacement::BottomDocked,
    ] {
        let backend = VT100Backend::with_scrollback(
            screen_size.width,
            screen_size.height,
            /*scrollback_len*/ 32,
        );
        let mut terminal = Terminal::with_options(backend)?;
        terminal.set_viewport_area(Rect::new(
            /*x*/ 0,
            /*y*/ 4,
            screen_size.width,
            /*height*/ 8,
        ));
        queue!(
            terminal.backend_mut(),
            MoveTo(/*x*/ 0, /*y*/ 2),
            Print("history-one\r\nhistory-two")
        )?;
        terminal.note_history_rows_inserted(/*inserted_rows*/ 2);
        let mut pending_history = vec![PendingHistoryLines {
            lines: plain_hyperlink_lines(vec![Line::from("queued history")]),
            wrap_policy: crate::insert_history::HistoryLineWrapPolicy::PreWrap,
        }];
        InlineViewportFrame {
            height: 4,
            screen_size,
            placement,
            role: InlineViewportRole::Persistent,
            previous_role: InlineViewportRole::Transient,
        }
        .prepare(
            &mut terminal,
            &mut pending_history,
            ScrollbackStrategy::FullScreen,
        )?;
        let actual = terminal
            .backend()
            .vt100()
            .screen()
            .rows(/*start*/ 0, screen_size.width)
            .map(|row| row.trim_end().to_owned())
            .collect::<Vec<_>>();
        let expected = [
            "",
            "",
            "",
            "",
            "",
            "history-one",
            "history-two",
            "queued history",
            "",
            "",
            "",
            "",
        ]
        .map(str::to_owned);
        assert_eq!(actual, expected, "{placement:?}");
        assert_eq!(
            terminal.viewport_area,
            Rect::new(
                /*x*/ 0,
                /*y*/ 8,
                screen_size.width,
                /*height*/ 4
            )
        );
        assert!(pending_history.is_empty());
    }
    Ok(())
}
