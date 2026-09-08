use std::io;
use std::io::stdout;

use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;
use ratatui::layout::Size;

use super::Tui;
use super::scrollback::ScrollbackStrategy;
use super::terminal_stderr::TerminalStderrGuard;
use crate::custom_terminal::Terminal;

/// Select deterministic history behavior when testing viewport geometry across terminal hosts.
#[derive(Clone, Copy)]
pub(crate) enum TestScrollback {
    Host,
    Standard,
    FullScreen,
}

pub(crate) fn make_test_tui() -> io::Result<Tui> {
    make_test_tui_with_scrollback(TestScrollback::Host)
}

pub(crate) fn make_test_tui_with_scrollback(scrollback: TestScrollback) -> io::Result<Tui> {
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::with_screen_size_and_cursor_position_for_test(
        backend,
        Size {
            width: 80,
            height: 24,
        },
        Position { x: 0, y: 0 },
    );
    let stderr_guard = TerminalStderrGuard::install()?;
    let mut tui = Tui::new(
        terminal,
        /*enhanced_keys_supported*/ false,
        stderr_guard,
    );
    match scrollback {
        TestScrollback::Host => {}
        TestScrollback::Standard => tui.scrollback = ScrollbackStrategy::Standard,
        TestScrollback::FullScreen => tui.scrollback = ScrollbackStrategy::FullScreen,
    }
    Ok(tui)
}
