//! A live task status row rendered above the composer while the agent is busy.
//!
//! The row owns motion timing, the optional interrupt hint, and short inline
//! context (for example, the unified-exec background-process summary). Short
//! details join the primary row when space allows; narrower layouts disclose
//! controls and details on bounded continuation rows.

use std::time::Duration;
use std::time::Instant;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::key_hint::ShortcutHint;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::motion::MotionMode;
use crate::motion::shimmer_text;
use crate::render::renderable::Renderable;
use crate::style::brand_style;
use crate::style::key_hint_style;
use crate::style::secondary_style;
use crate::text_formatting::capitalize_first;
use crate::tui::FrameRequester;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

pub(crate) const STATUS_DETAILS_DEFAULT_MAX_LINES: usize = 3;
const STATUS_MARKER: &str = "✦";
const SEGMENT_SEPARATOR: &str = " · ";
const METADATA_GAP: &str = "  ";
const DETAILS_PREFIX: &str = "  └ ";
const DETAILS_BRANCH_PREFIX: &str = "  ├ ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusDetailsCapitalization {
    CapitalizeFirst,
    Preserve,
}

struct StatusLayout<'a> {
    header: &'a str,
    inline_details: Option<&'a str>,
    metadata: StatusMetadataLayout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InterruptHintFormat {
    Full,
    Compact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatusMetadataLayout {
    Full,
    Elapsed,
    CompactInterrupt,
    InterruptContinuation,
    PrimaryOnly,
}

/// Displays a single-line in-progress status with optional wrapped details.
pub(crate) struct StatusIndicatorWidget {
    /// Animated header text (defaults to "Working").
    header: String,
    details: Option<String>,
    details_max_lines: usize,
    /// Optional subordinate activity rendered below the elapsed/interrupt segment.
    inline_message: Option<String>,
    show_interrupt_hint: bool,
    interrupt_binding: Option<ShortcutHint>,

    elapsed_running: Duration,
    last_resume_at: Instant,
    is_paused: bool,
    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,
    animations_enabled: bool,
}

// Format elapsed seconds into a compact human-friendly form used by the status line.
// Examples: 0s, 59s, 1m 00s, 59m 59s, 1h 00m 00s, 2h 03m 09s
pub fn fmt_elapsed_compact(elapsed_secs: u64) -> String {
    if elapsed_secs < 60 {
        return format!("{elapsed_secs}s");
    }
    if elapsed_secs < 3600 {
        let minutes = elapsed_secs / 60;
        let seconds = elapsed_secs % 60;
        return format!("{minutes}m {seconds:02}s");
    }
    let hours = elapsed_secs / 3600;
    let minutes = (elapsed_secs % 3600) / 60;
    let seconds = elapsed_secs % 60;
    format!("{hours}h {minutes:02}m {seconds:02}s")
}

impl StatusIndicatorWidget {
    pub(crate) fn new(
        app_event_tx: AppEventSender,
        frame_requester: FrameRequester,
        animations_enabled: bool,
    ) -> Self {
        Self {
            header: String::from("Working"),
            details: None,
            details_max_lines: STATUS_DETAILS_DEFAULT_MAX_LINES,
            inline_message: None,
            show_interrupt_hint: true,
            interrupt_binding: Some(key_hint::plain(KeyCode::Esc).into()),
            elapsed_running: Duration::ZERO,
            last_resume_at: Instant::now(),
            is_paused: false,

            app_event_tx,
            frame_requester,
            animations_enabled,
        }
    }

    pub(crate) fn interrupt(&self) {
        self.app_event_tx.interrupt();
    }

    /// Update the animated primary status label.
    pub(crate) fn update_header(&mut self, header: String) {
        self.header = header;
    }

    /// Update the details text shown below the header.
    pub(crate) fn update_details(
        &mut self,
        details: Option<String>,
        capitalization: StatusDetailsCapitalization,
        max_lines: usize,
    ) {
        self.details_max_lines = max_lines.max(1);
        self.details = details
            .map(|details| details.trim().to_string())
            .filter(|details| !details.is_empty())
            .map(|details| match capitalization {
                StatusDetailsCapitalization::CapitalizeFirst => capitalize_first(&details),
                StatusDetailsCapitalization::Preserve => details,
            });
    }

    /// Update the subordinate activity shown below the elapsed/interrupt hint.
    ///
    /// Callers should provide plain, already-contextualized text. Passing
    /// verbose status prose here can cause frequent width truncation and hide
    /// the more important elapsed/interrupt hint.
    pub(crate) fn update_inline_message(&mut self, message: Option<String>) {
        self.inline_message = message
            .map(|message| message.trim().to_string())
            .filter(|message| !message.is_empty());
    }

    pub(crate) fn header(&self) -> &str {
        &self.header
    }

    #[cfg(test)]
    pub(crate) fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    pub(crate) fn set_interrupt_hint_visible(&mut self, visible: bool) {
        self.show_interrupt_hint = visible;
    }

    pub(crate) fn set_interrupt_binding(&mut self, binding: Option<ShortcutHint>) {
        self.interrupt_binding = binding;
    }

    pub(crate) fn pause_timer(&mut self) {
        self.pause_timer_at(Instant::now());
    }

    pub(crate) fn resume_timer(&mut self) {
        self.resume_timer_at(Instant::now());
    }

    pub(crate) fn pause_timer_at(&mut self, now: Instant) {
        if self.is_paused {
            return;
        }
        self.elapsed_running += now.saturating_duration_since(self.last_resume_at);
        self.is_paused = true;
    }

    pub(crate) fn resume_timer_at(&mut self, now: Instant) {
        if !self.is_paused {
            return;
        }
        self.last_resume_at = now;
        self.is_paused = false;
        self.frame_requester.schedule_frame();
    }

    fn elapsed_duration_at(&self, now: Instant) -> Duration {
        let mut elapsed = self.elapsed_running;
        if !self.is_paused {
            elapsed += now.saturating_duration_since(self.last_resume_at);
        }
        elapsed
    }

    fn elapsed_seconds_at(&self, now: Instant) -> u64 {
        self.elapsed_duration_at(now).as_secs()
    }

    pub fn elapsed_seconds(&self) -> u64 {
        self.elapsed_seconds_at(Instant::now())
    }

    fn interrupt_hint_spans(&self, format: InterruptHintFormat) -> Option<Vec<Span<'static>>> {
        if !self.show_interrupt_hint {
            return None;
        }
        let interrupt_binding = self.interrupt_binding?;
        let binding = if interrupt_binding == ShortcutHint::from(key_hint::plain(KeyCode::Esc)) {
            Span::styled("Esc", key_hint_style())
        } else {
            interrupt_binding.into()
        };
        let label = match format {
            InterruptHintFormat::Full => " interrupt",
            InterruptHintFormat::Compact => " stop",
        };
        Some(vec![binding, Span::styled(label, secondary_style())])
    }

    fn status_layout<'a>(&'a self, width: u16, pretty_elapsed: &str) -> StatusLayout<'a> {
        let width = usize::from(width);
        let full_interrupt_width = self
            .interrupt_hint_spans(InterruptHintFormat::Full)
            .map(|spans| Line::from(spans).width())
            .filter(|interrupt_width| *interrupt_width > 0);
        let compact_interrupt_width = self
            .interrupt_hint_spans(InterruptHintFormat::Compact)
            .map(|spans| Line::from(spans).width())
            .filter(|interrupt_width| *interrupt_width > 0);
        let prefix_width = UnicodeWidthStr::width(STATUS_MARKER) + 1;
        let separator_width = UnicodeWidthStr::width(SEGMENT_SEPARATOR);
        let metadata_gap_width = UnicodeWidthStr::width(METADATA_GAP);
        let elapsed_width = UnicodeWidthStr::width(pretty_elapsed);
        let essential_metadata_width = compact_interrupt_width.unwrap_or(elapsed_width);

        let full_header = self.header.as_str();
        let header = if full_header.starts_with("Waiting for ")
            && prefix_width
                + UnicodeWidthStr::width(full_header)
                + metadata_gap_width
                + essential_metadata_width
                > width
        {
            "Waiting"
        } else {
            full_header
        };
        let primary_width = prefix_width + UnicodeWidthStr::width(header);
        let metadata = match full_interrupt_width {
            Some(interrupt_width)
                if primary_width
                    + metadata_gap_width
                    + elapsed_width
                    + separator_width
                    + interrupt_width
                    <= width =>
            {
                StatusMetadataLayout::Full
            }
            Some(_)
                if compact_interrupt_width.is_some_and(|interrupt_width| {
                    primary_width + metadata_gap_width + interrupt_width <= width
                }) =>
            {
                StatusMetadataLayout::CompactInterrupt
            }
            Some(_) => StatusMetadataLayout::InterruptContinuation,
            None if primary_width + metadata_gap_width + elapsed_width <= width => {
                StatusMetadataLayout::Elapsed
            }
            None => StatusMetadataLayout::PrimaryOnly,
        };
        let reserved_metadata_width = match metadata {
            StatusMetadataLayout::Full => {
                metadata_gap_width
                    + elapsed_width
                    + separator_width
                    + full_interrupt_width.unwrap_or(0)
            }
            StatusMetadataLayout::Elapsed => metadata_gap_width + elapsed_width,
            StatusMetadataLayout::CompactInterrupt => {
                metadata_gap_width + compact_interrupt_width.unwrap_or(0)
            }
            StatusMetadataLayout::InterruptContinuation | StatusMetadataLayout::PrimaryOnly => 0,
        };
        let inline_details = self.details.as_deref().filter(|details| {
            matches!(
                metadata,
                StatusMetadataLayout::Full | StatusMetadataLayout::Elapsed
            ) && !details.contains('\n')
                && primary_width
                    + separator_width
                    + UnicodeWidthStr::width(*details)
                    + reserved_metadata_width
                    <= width
        });

        StatusLayout {
            header,
            inline_details,
            metadata,
        }
    }

    fn inline_message_lines(&self, width: u16, has_following_details: bool) -> Vec<Line<'static>> {
        let Some(message) = self.inline_message.as_deref() else {
            return Vec::new();
        };
        let width = usize::from(width);
        let prefix_width = UnicodeWidthStr::width(DETAILS_PREFIX);
        let terminal_controls = " · /ps inspect · /stop terminate";
        let compact_message = message.replace(" running ·", " ·");
        let message = if prefix_width + UnicodeWidthStr::width(message) <= width {
            message.to_string()
        } else {
            compact_message
        };

        if prefix_width + UnicodeWidthStr::width(message.as_str()) <= width {
            let prefix = if has_following_details {
                DETAILS_BRANCH_PREFIX
            } else {
                DETAILS_PREFIX
            };
            return vec![Line::from(vec![prefix.dim(), message.dim()])];
        }

        if self
            .inline_message
            .as_deref()
            .is_some_and(|message| message.ends_with(terminal_controls))
        {
            let subject = if message.starts_with("Terminal ") {
                "Terminal"
            } else {
                "Terminals"
            };
            let parts = [subject, "/ps inspect", "/stop terminate"];
            return parts
                .into_iter()
                .enumerate()
                .map(|(idx, part)| {
                    let has_following = idx + 1 < parts.len() || has_following_details;
                    let prefix = if has_following {
                        DETAILS_BRANCH_PREFIX
                    } else {
                        DETAILS_PREFIX
                    };
                    truncate_line_with_ellipsis_if_overflow(
                        Line::from(vec![prefix.dim(), part.dim()]),
                        width,
                    )
                })
                .collect();
        }

        let prefix = if has_following_details {
            DETAILS_BRANCH_PREFIX
        } else {
            DETAILS_PREFIX
        };
        vec![truncate_line_with_ellipsis_if_overflow(
            Line::from(vec![prefix.dim(), message.dim()]),
            width,
        )]
    }

    /// Wrap the details text into a fixed width and return the lines, truncating if necessary.
    fn wrapped_details_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(details) = self.details.as_deref() else {
            return Vec::new();
        };
        if width == 0 {
            return Vec::new();
        }

        let prefix_width = UnicodeWidthStr::width(DETAILS_PREFIX);
        let content_width = usize::from(width).saturating_sub(prefix_width).max(1);
        let initial_wrap = textwrap::wrap(details, content_width);
        let wrap_width = if !details.contains('\n')
            && initial_wrap.len() == 2
            && initial_wrap[1].split_whitespace().count() == 1
        {
            initial_wrap[0]
                .rfind(char::is_whitespace)
                .map(|split_at| UnicodeWidthStr::width(&initial_wrap[0][..split_at]))
                .filter(|candidate_width| *candidate_width > 0)
                .filter(|candidate_width| {
                    let candidate = textwrap::wrap(details, *candidate_width);
                    candidate.len() == 2 && candidate[1].split_whitespace().count() > 1
                })
                .map_or(usize::from(width), |balanced_content_width| {
                    balanced_content_width + prefix_width
                })
        } else {
            usize::from(width)
        };
        let opts = RtOptions::new(wrap_width)
            .initial_indent(Line::from(DETAILS_PREFIX.dim()))
            .subsequent_indent(Line::from(Span::from(" ".repeat(prefix_width)).dim()))
            .break_words(/*break_words*/ true);

        let mut out = word_wrap_lines(details.lines().map(|line| vec![line.dim()]), opts);

        if out.len() > self.details_max_lines {
            out.truncate(self.details_max_lines);
            if let Some(last) = out.last_mut() {
                let mut ellipsized = last.clone();
                ellipsized.spans.push(Span::styled("…", secondary_style()));
                *last = truncate_line_with_ellipsis_if_overflow(ellipsized, usize::from(width));
            }
        }

        out
    }
}

impl Renderable for StatusIndicatorWidget {
    fn desired_height(&self, width: u16) -> u16 {
        let elapsed_seconds = self.elapsed_seconds();
        let layout_elapsed = fmt_elapsed_compact(elapsed_seconds.saturating_add(1));
        let layout = self.status_layout(width, &layout_elapsed);
        let details_height = if layout.inline_details.is_some() {
            0
        } else {
            u16::try_from(self.wrapped_details_lines(width).len()).unwrap_or(0)
        };
        let inline_message_height =
            u16::try_from(self.inline_message_lines(width, details_height > 0).len()).unwrap_or(0);
        1 + u16::from(layout.metadata == StatusMetadataLayout::InterruptContinuation)
            + inline_message_height
            + details_height
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        if self.animations_enabled {
            // Schedule next animation frame.
            self.frame_requester
                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
        }
        let now = Instant::now();
        let elapsed_seconds = self.elapsed_duration_at(now).as_secs();
        let pretty_elapsed = fmt_elapsed_compact(elapsed_seconds);
        let layout_elapsed = fmt_elapsed_compact(elapsed_seconds.saturating_add(1));
        let motion_mode = MotionMode::from_animations_enabled(self.animations_enabled);
        let layout = self.status_layout(area.width, &layout_elapsed);
        let full_interrupt_hint_spans = self.interrupt_hint_spans(InterruptHintFormat::Full);
        let compact_interrupt_hint_spans = self.interrupt_hint_spans(InterruptHintFormat::Compact);
        let mut action_spans = vec![Span::styled(STATUS_MARKER, brand_style()), " ".into()];
        action_spans.extend(shimmer_text(layout.header, motion_mode));
        if let Some(details) = layout.inline_details {
            action_spans.push(Span::styled(SEGMENT_SEPARATOR, secondary_style()));
            action_spans.push(Span::styled(details.to_string(), secondary_style()));
        }
        let mut metadata_spans = Vec::new();
        match layout.metadata {
            StatusMetadataLayout::Full => {
                metadata_spans.push(Span::styled(pretty_elapsed, secondary_style()));
                if let Some(interrupt_hint_spans) = full_interrupt_hint_spans.as_ref() {
                    metadata_spans.push(Span::styled(SEGMENT_SEPARATOR, secondary_style()));
                    metadata_spans.extend(interrupt_hint_spans.clone());
                }
            }
            StatusMetadataLayout::Elapsed => {
                metadata_spans.push(Span::styled(pretty_elapsed, secondary_style()));
            }
            StatusMetadataLayout::CompactInterrupt => {
                if let Some(interrupt_hint_spans) = compact_interrupt_hint_spans.as_ref() {
                    metadata_spans.extend(interrupt_hint_spans.clone());
                }
            }
            StatusMetadataLayout::InterruptContinuation | StatusMetadataLayout::PrimaryOnly => {}
        }
        let metadata_width = Line::from(metadata_spans.clone()).width();
        let area_width = usize::from(area.width);
        let metadata_gap_width = if metadata_width > 0 {
            UnicodeWidthStr::width(METADATA_GAP)
        } else {
            0
        };
        let action_width =
            area_width.saturating_sub(metadata_width.saturating_add(metadata_gap_width));
        let mut status_line =
            truncate_line_with_ellipsis_if_overflow(Line::from(action_spans), action_width);
        if metadata_width > 0 {
            let padding_width =
                area_width.saturating_sub(status_line.width().saturating_add(metadata_width));
            status_line.spans.push(Span::raw(" ".repeat(padding_width)));
            status_line.spans.extend(metadata_spans);
        }
        let mut lines = Vec::new();
        lines.push(truncate_line_with_ellipsis_if_overflow(
            status_line,
            usize::from(area.width),
        ));
        let details = if layout.inline_details.is_some() {
            Vec::new()
        } else {
            self.wrapped_details_lines(area.width)
        };
        let inline_message_lines = self.inline_message_lines(area.width, !details.is_empty());
        if area.height > 1 {
            if layout.metadata == StatusMetadataLayout::InterruptContinuation
                && let Some(interrupt_hint_spans) = full_interrupt_hint_spans.as_ref()
            {
                let prefix = if !inline_message_lines.is_empty() || !details.is_empty() {
                    DETAILS_BRANCH_PREFIX
                } else {
                    DETAILS_PREFIX
                };
                let mut spans = vec![prefix.dim()];
                spans.extend(interrupt_hint_spans.clone());
                lines.push(truncate_line_with_ellipsis_if_overflow(
                    Line::from(spans),
                    usize::from(area.width),
                ));
            }
            lines.extend(inline_message_lines);
            let max_details = usize::from(area.height).saturating_sub(lines.len());
            lines.extend(details.into_iter().take(max_details));
        }

        Paragraph::new(Text::from(lines)).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Duration;
    use std::time::Instant;
    use tokio::sync::mpsc::unbounded_channel;

    use pretty_assertions::assert_eq;

    #[test]
    fn fmt_elapsed_compact_formats_seconds_minutes_hours() {
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 0), "0s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 1), "1s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 59), "59s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 60), "1m 00s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 61), "1m 01s");
        assert_eq!(fmt_elapsed_compact(3 * 60 + 5), "3m 05s");
        assert_eq!(fmt_elapsed_compact(59 * 60 + 59), "59m 59s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 3600), "1h 00m 00s");
        assert_eq!(fmt_elapsed_compact(3600 + 60 + 1), "1h 01m 01s");
        assert_eq!(fmt_elapsed_compact(25 * 3600 + 2 * 60 + 3), "25h 02m 03s");
    }

    #[test]
    fn renders_with_working_header() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        let viewport_width = 120;
        let viewport_height = 36;
        let status_height = w.desired_height(viewport_width);
        let mut terminal =
            Terminal::new(TestBackend::new(viewport_width, viewport_height)).expect("terminal");
        terminal
            .draw(|f| {
                w.render(
                    Rect::new(/*x*/ 0, /*y*/ 0, viewport_width, status_height),
                    f.buffer_mut(),
                )
            })
            .expect("draw");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .chunks(usize::from(viewport_width))
            .take(usize::from(status_height))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(format!("viewport 120x36:\n{rendered}"));
    }

    #[test]
    fn renders_short_details_inline_at_normal_widths() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.update_details(
            Some("Searching for cache defects".to_string()),
            StatusDetailsCapitalization::Preserve,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        let mut snapshots = Vec::new();
        for width in [80, 120] {
            assert_eq!(w.desired_height(width), 1);
            let mut terminal = Terminal::new(TestBackend::new(width, 1)).expect("terminal");
            terminal
                .draw(|f| w.render(f.area(), f.buffer_mut()))
                .expect("draw");
            let rendered = terminal.backend().buffer().content()[..usize::from(width)]
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string();
            snapshots.push(format!("width {width}:\n{rendered}"));
        }

        insta::assert_snapshot!(snapshots.join("\n\n"));
    }

    #[test]
    fn renders_truncated() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        assert_eq!(
            w.status_layout(/*width*/ 20, "0s").metadata,
            StatusMetadataLayout::CompactInterrupt
        );
        assert_eq!(w.desired_height(/*width*/ 20), 1);
        let mut terminal = Terminal::new(TestBackend::new(20, 1)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_wrapped_details_panama_two_lines() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.update_details(
            Some("A man a plan a canal panama".to_string()),
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        w.set_interrupt_hint_visible(/*visible*/ false);

        // Freeze time-dependent rendering (elapsed + spinner) to keep the snapshot stable.
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        // Prefix is 4 columns, so a width of 30 yields a content width of 26: one column
        // short of fitting the whole phrase (27 cols), forcing exactly one wrap without ellipsis.
        let mut terminal = Terminal::new(TestBackend::new(30, 3)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .chunks(30)
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                format!("✦ Working{}0s", " ".repeat(19)),
                "  └ A man a plan a".to_string(),
                "    canal panama".to_string(),
            ]
        );
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn three_line_long_word_details_remain_bounded_and_complete() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.update_details(
            Some("abcdefghijklmnopqr".to_string()),
            StatusDetailsCapitalization::Preserve,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );

        let lines = w.wrapped_details_lines(/*width*/ 10);
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| line.width() <= 10));
        assert_eq!(
            lines
                .iter()
                .flat_map(|line| line.spans.iter().skip(1))
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            "abcdefghijklmnopqr"
        );
    }

    #[test]
    fn narrow_wait_status_preserves_terminal_controls_and_detail_hierarchy() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.update_header("Waiting for background terminal".to_string());
        w.update_inline_message(Some(
            "Terminal running · /ps inspect · /stop terminate".to_string(),
        ));
        w.update_details(
            Some("cargo test".to_string()),
            StatusDetailsCapitalization::Preserve,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        assert_eq!(w.desired_height(/*width*/ 47), 3);
        let mut terminal = Terminal::new(TestBackend::new(47, 3)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .chunks(47)
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered,
            vec![
                "✦ Waiting for background terminal      Esc stop".to_string(),
                "  ├ Terminal · /ps inspect · /stop terminate".to_string(),
                "  └ cargo test".to_string(),
            ]
        );
    }

    #[test]
    fn waiting_terminal_controls_remain_available_across_widths() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.update_header("Waiting for background terminal".to_string());
        w.update_inline_message(Some(
            "Terminal running · /ps inspect · /stop terminate".to_string(),
        ));
        w.update_details(
            Some("cargo test".to_string()),
            StatusDetailsCapitalization::Preserve,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        let mut snapshots = Vec::new();
        for width in [20, 47, 80, 120] {
            let height = w.desired_height(width);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
            terminal
                .draw(|f| w.render(f.area(), f.buffer_mut()))
                .expect("draw");
            let rendered = terminal
                .backend()
                .buffer()
                .content()
                .chunks(usize::from(width))
                .map(|row| {
                    row.iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>()
                        .trim_end()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(rendered.contains("/ps inspect"));
            assert!(rendered.contains("/stop terminate"));
            snapshots.push(format!("width {width}:\n{rendered}"));
        }

        insta::assert_snapshot!(snapshots.join("\n\n"));
    }

    #[test]
    fn renders_without_spinner_when_animations_disabled() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        let line = terminal.backend().buffer().content()[..80]
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();

        let line = line.trim_end();
        assert!(line.starts_with("✦ Working"));
        assert!(line.ends_with("0s · Esc interrupt"));
    }

    #[test]
    fn renders_remapped_interrupt_hint() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
        );
        w.set_interrupt_binding(Some(key_hint::plain(KeyCode::F(12)).into()));
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn timer_pauses_when_requested() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut widget = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
        );

        let baseline = Instant::now();
        widget.last_resume_at = baseline;

        let before_pause = widget.elapsed_seconds_at(baseline + Duration::from_secs(5));
        assert_eq!(before_pause, 5);

        widget.pause_timer_at(baseline + Duration::from_secs(5));
        let paused_elapsed = widget.elapsed_seconds_at(baseline + Duration::from_secs(10));
        assert_eq!(paused_elapsed, before_pause);

        widget.resume_timer_at(baseline + Duration::from_secs(10));
        let after_resume = widget.elapsed_seconds_at(baseline + Duration::from_secs(13));
        assert_eq!(after_resume, before_pause + 3);
    }

    #[test]
    fn details_overflow_adds_ellipsis() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
        );
        w.update_details(
            Some("abcd abcd abcd abcd".to_string()),
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );

        let lines = w.wrapped_details_lines(/*width*/ 6);
        assert_eq!(lines.len(), STATUS_DETAILS_DEFAULT_MAX_LINES);
        let last = lines.last().expect("expected last details line");
        assert!(
            last.spans
                .last()
                .is_some_and(|span| span.content.as_ref().ends_with('…')),
            "expected ellipsis in last line: {last:?}"
        );
    }

    #[test]
    fn details_args_can_disable_capitalization_and_limit_lines() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
        );
        w.update_details(
            Some("  cargo test -p codex-core and then cargo test -p codex-tui  ".to_string()),
            StatusDetailsCapitalization::Preserve,
            /*max_lines*/ 1,
        );

        assert_eq!(
            w.details(),
            Some("cargo test -p codex-core and then cargo test -p codex-tui")
        );

        let lines = w.wrapped_details_lines(/*width*/ 24);
        assert_eq!(lines.len(), 1);
        let last = lines.last().expect("expected one details line");
        assert!(
            last.spans
                .last()
                .is_some_and(|span| span.content.as_ref().contains('…')),
            "expected one-line details to be ellipsized, got {last:?}"
        );
    }
}
