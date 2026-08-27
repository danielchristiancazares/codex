use std::num::NonZeroU16;

use divan::Bencher;
use ratatui::buffer::Buffer;
use ratatui::buffer::CellDiffOption;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

mod custom_terminal {
    #![allow(dead_code, unused_imports)]

    include!("../src/custom_terminal.rs");

    pub(super) fn diff_count(previous: &Buffer, next: &Buffer) -> usize {
        diff_buffers(previous, next).len()
    }

    pub(super) fn serialize(previous: &Buffer, next: &Buffer, output: &mut Vec<u8>) -> usize {
        output.clear();
        let commands = diff_buffers(previous, next);
        draw(output, commands.into_iter(), CursorPositioning::Predicted)
            .expect("Vec writes should succeed");
        output.len()
    }
}

const WIDTH: u16 = 120;
const HEIGHT: u16 = 40;
const LINK_X: u16 = 4;
const LINK_Y: u16 = 36;

fn main() {
    divan::main();
}

#[divan::bench(sample_count = 50, sample_size = 500)]
fn buffer_diff_unchanged(bencher: Bencher) {
    let previous = transcript_buffer();
    let next = previous.clone();
    bencher.bench_local(|| {
        custom_terminal::diff_count(divan::black_box(&previous), divan::black_box(&next))
    });
}

#[divan::bench(sample_count = 50, sample_size = 500)]
fn buffer_diff_sparse_update(bencher: Bencher) {
    let (previous, next) = sparse_buffers();
    bencher.bench_local(|| {
        custom_terminal::diff_count(divan::black_box(&previous), divan::black_box(&next))
    });
}

#[divan::bench(sample_count = 30, sample_size = 100)]
fn buffer_diff_dense_repaint(bencher: Bencher) {
    let (previous, next) = dense_buffers();
    bencher.bench_local(|| {
        custom_terminal::diff_count(divan::black_box(&previous), divan::black_box(&next))
    });
}

#[divan::bench(sample_count = 50, sample_size = 500)]
fn ansi_sparse_update(bencher: Bencher) {
    let (previous, next) = sparse_buffers();
    let mut output = Vec::with_capacity(/*capacity*/ 256);
    bencher.bench_local(|| {
        custom_terminal::serialize(
            divan::black_box(&previous),
            divan::black_box(&next),
            &mut output,
        )
    });
}

#[divan::bench(sample_count = 30, sample_size = 100)]
fn ansi_dense_repaint(bencher: Bencher) {
    let (previous, next) = dense_buffers();
    let mut output = Vec::with_capacity(/*capacity*/ 16 * 1024);
    bencher.bench_local(|| {
        custom_terminal::serialize(
            divan::black_box(&previous),
            divan::black_box(&next),
            &mut output,
        )
    });
}

#[divan::bench(sample_count = 50, sample_size = 500)]
fn ansi_hyperlink_update(bencher: Bencher) {
    let mut previous = transcript_buffer();
    set_hyperlink(
        &mut previous,
        LINK_X,
        LINK_Y,
        "https://example.com/build/1234",
        "build 1234",
    );
    let mut next = previous.clone();
    set_hyperlink(
        &mut next,
        LINK_X,
        LINK_Y,
        "https://example.com/build/12345",
        "build 12345",
    );
    let mut output = Vec::with_capacity(/*capacity*/ 512);
    bencher.bench_local(|| {
        custom_terminal::serialize(
            divan::black_box(&previous),
            divan::black_box(&next),
            &mut output,
        )
    });
}

fn sparse_buffers() -> (Buffer, Buffer) {
    let previous = transcript_buffer();
    let mut next = previous.clone();
    next[(31, 37)].set_char('8').set_fg(Color::Cyan);
    (previous, next)
}

fn dense_buffers() -> (Buffer, Buffer) {
    let previous = transcript_buffer();
    let mut next = previous.clone();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            next[(x, y)]
                .set_char(if (x + y) % 7 == 0 { '#' } else { ' ' })
                .set_fg(Color::Yellow)
                .set_bg(Color::Blue);
        }
    }
    (previous, next)
}

fn transcript_buffer() -> Buffer {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ WIDTH, /*height*/ HEIGHT,
    );
    let mut buffer = Buffer::empty(area);
    let body = [
        "* Explored",
        "  `- Read custom_terminal.rs",
        "* Comparing previous and current buffers before emitting ANSI updates.",
        "",
        "* Ran just test -p codex-tui custom_terminal::tests",
        "  `- tests passed",
    ];

    for y in 0..HEIGHT {
        let text = body[usize::from(y) % body.len()];
        let style = match y % 6 {
            0 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            1 => Style::default().fg(Color::DarkGray),
            4 => Style::default().fg(Color::Green),
            5 => Style::default().fg(Color::DarkGray),
            _ => Style::default(),
        };
        buffer.set_string(/*x*/ 0, /*y*/ y, text, style);
    }

    buffer.set_string(
        /*x*/ 0,
        /*y*/ 37,
        "Working (38s - esc to interrupt)",
        Style::default().fg(Color::Cyan),
    );
    buffer
}

fn set_hyperlink(buffer: &mut Buffer, x: u16, y: u16, destination: &str, visible: &str) {
    let symbol = format!("\x1b]8;;{destination}\x07{visible}\x1b]8;;\x07");
    let width = u16::try_from(visible.len()).expect("visible hyperlink width should fit in u16");
    buffer[(x, y)]
        .set_symbol(&symbol)
        .set_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        )
        .set_diff_option(CellDiffOption::ForcedWidth(
            NonZeroU16::new(width).expect("visible hyperlink width should be nonzero"),
        ));
}
