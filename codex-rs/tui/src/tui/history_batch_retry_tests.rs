use super::PendingHistoryLines;
use super::Tui;
use super::scrollback::ScrollbackStrategy;
use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::test_backend::VT100Backend;
use pretty_assertions::assert_eq;
use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::backend::WindowSize;
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;
use std::io;
use std::io::Write;
use std::ops::Range;

struct FailOnceBackend {
    inner: VT100Backend,
    marker: &'static [u8],
    failed: bool,
}

impl FailOnceBackend {
    fn new(width: u16, height: u16, marker: &'static [u8]) -> Self {
        Self {
            inner: VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 32),
            marker,
            failed: false,
        }
    }

    fn contents_with_scrollback(&self) -> String {
        let mut screen = self.inner.vt100().screen().clone();
        screen.set_scrollback(/*rows*/ usize::MAX);
        screen.contents()
    }
}

impl Write for FailOnceBackend {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.failed
            && buf
                .windows(self.marker.len())
                .any(|window| window == self.marker)
        {
            self.failed = true;
            return Err(io::Error::other("injected history write failure"));
        }
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut self.inner)
    }
}

impl Backend for FailOnceBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn append_lines(&mut self, line_count: u16) -> io::Result<()> {
        self.inner.append_lines(line_count)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }

    fn scroll_region_up(&mut self, region: Range<u16>, scroll_by: u16) -> io::Result<()> {
        self.inner.scroll_region_up(region, scroll_by)
    }

    fn scroll_region_down(&mut self, region: Range<u16>, scroll_by: u16) -> io::Result<()> {
        self.inner.scroll_region_down(region, scroll_by)
    }
}

#[test]
fn successful_history_batches_are_acknowledged_before_a_later_batch_is_retried() {
    let width = 40;
    let height = 8;
    let backend = FailOnceBackend::new(width, height, b"unwritten batch");
    let mut terminal = Terminal::with_options(backend).expect("terminal");
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ height - 2,
        width,
        /*height*/ 2,
    ));
    let committed_lines = plain_hyperlink_lines(vec![Line::from("committed batch")]);
    let unwritten_lines = plain_hyperlink_lines(vec![Line::from("unwritten batch")]);
    let mut pending = vec![
        PendingHistoryLines {
            lines: committed_lines,
            wrap_policy: HistoryLineWrapPolicy::PreWrap,
        },
        PendingHistoryLines {
            lines: unwritten_lines.clone(),
            wrap_policy: HistoryLineWrapPolicy::Terminal,
        },
    ];
    let screen_size = terminal.last_known_screen_size;

    let error = Tui::flush_pending_history_lines(
        &mut terminal,
        &mut pending,
        ScrollbackStrategy::Standard,
        screen_size,
    )
    .expect_err("second batch should hit the injected write failure");
    assert_eq!(error.to_string(), "injected history write failure");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].lines, unwritten_lines);

    Tui::flush_pending_history_lines(
        &mut terminal,
        &mut pending,
        ScrollbackStrategy::Standard,
        screen_size,
    )
    .expect("retry unwritten batch");

    assert!(pending.is_empty());
    let contents = terminal.backend().contents_with_scrollback();
    assert_eq!(contents.matches("committed batch").count(), 1, "{contents}");
    assert_eq!(contents.matches("unwritten batch").count(), 1, "{contents}");
}
