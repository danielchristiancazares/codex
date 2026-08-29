use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;

use crate::key_hint;
use crate::line_truncation::truncate_line_to_width;
use crate::render::renderable::Renderable;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

/// Widget that displays pending steers plus follow-up inputs held while a turn is in progress.
///
/// Rejected steers get first claim on its fixed row budget, followed by active
/// steers and queued messages; counts and configured hints disclose the rest.
pub(crate) struct PendingInputPreview {
    pub pending_steers: Vec<String>,
    pub rejected_steers: Vec<String>,
    pub queued_messages: Vec<String>,
    /// Key combination rendered in the hint line.  Defaults to Alt+Up but may
    /// be overridden for terminals where that chord is unavailable.
    edit_binding: Option<key_hint::ShortcutHint>,
    /// Key combination rendered for immediately interrupting and sending steers.
    interrupt_binding: Option<key_hint::ShortcutHint>,
}

/// Supporting context gets eight rows total, including counts and action hints.
const VISIBLE_ROW_CAP: usize = 8;
const ITEM_ROW_CAP: usize = 2;

#[derive(Clone, Copy)]
enum PreviewKind {
    Rejected,
    Pending,
    Queued,
}

impl PendingInputPreview {
    pub(crate) fn new() -> Self {
        Self {
            pending_steers: Vec::new(),
            rejected_steers: Vec::new(),
            queued_messages: Vec::new(),
            edit_binding: Some(key_hint::alt(KeyCode::Up).into()),
            interrupt_binding: Some(key_hint::plain(KeyCode::Esc).into()),
        }
    }

    /// Replace the keybinding shown in the hint line at the bottom of the
    /// queued-messages list.  The caller is responsible for also wiring the
    /// corresponding key event handler.
    pub(crate) fn set_edit_binding(&mut self, binding: Option<key_hint::ShortcutHint>) {
        self.edit_binding = binding;
    }

    pub(crate) fn set_interrupt_binding(&mut self, binding: Option<key_hint::ShortcutHint>) {
        self.interrupt_binding = binding;
    }

    fn title_lines(&self, width: u16) -> Vec<Line<'static>> {
        let rejected = self.rejected_steers.len();
        let pending = self.pending_steers.len();
        let queued = self.queued_messages.len();
        let populated_categories = [rejected, pending, queued]
            .into_iter()
            .filter(|count| *count > 0)
            .count();
        let mut spans = vec!["• ".dim()];

        match (populated_categories, rejected, pending, queued) {
            (1, rejected, 0, 0) => spans.extend(vec![
                format!(
                    "{rejected} {}",
                    if rejected == 1 { "retry" } else { "retries" }
                )
                .red(),
                " at turn end".into(),
            ]),
            (1, 0, pending, 0) => {
                spans.push(format!("{pending} steer").cyan());
                if pending != 1 {
                    spans.push("s".cyan());
                }
                if let Some(binding) = self.interrupt_binding {
                    spans.extend(vec![" · ".dim(), binding.into(), " send steers".dim()]);
                }
            }
            (1, 0, 0, queued) => {
                spans.push(format!("{queued} queued").into());
                if let Some(binding) = self.edit_binding {
                    spans.extend(vec![
                        " · ".dim(),
                        binding.into(),
                        " edit latest queued".dim(),
                    ]);
                }
            }
            _ if width >= 48 => {
                if rejected > 0 {
                    spans.push(
                        format!(
                            "{rejected} {}",
                            if rejected == 1 { "retry" } else { "retries" }
                        )
                        .red(),
                    );
                }
                if pending > 0 {
                    if spans.len() > 1 {
                        spans.push(" · ".dim());
                    }
                    spans.push(format!("{pending} steer").cyan());
                    if pending != 1 {
                        spans.push("s".cyan());
                    }
                }
                if queued > 0 {
                    if spans.len() > 1 {
                        spans.push(" · ".dim());
                    }
                    spans.push(format!("{queued} queued").into());
                }
            }
            _ => spans.push(format!("{} pending inputs", rejected + pending + queued).into()),
        }

        adaptive_wrap_lines(
            std::iter::once(Line::from(spans)),
            RtOptions::new(width as usize).subsequent_indent(Line::from("  ".dim())),
        )
        .into_iter()
        .take(ITEM_ROW_CAP)
        .collect()
    }

    fn action_lines(&self, width: u16) -> Vec<Line<'static>> {
        let categories = [
            !self.rejected_steers.is_empty(),
            !self.pending_steers.is_empty(),
            !self.queued_messages.is_empty(),
        ]
        .into_iter()
        .filter(|populated| *populated)
        .count();
        if categories < 2 {
            return Vec::new();
        }

        let mut spans = vec!["  ".into()];
        if let Some(binding) = self
            .interrupt_binding
            .filter(|_| !self.pending_steers.is_empty())
        {
            spans.extend(vec![binding.into(), " steers".dim()]);
        }
        if let Some(binding) = self
            .edit_binding
            .filter(|_| !self.queued_messages.is_empty())
        {
            if spans.len() > 1 {
                spans.push(" · ".dim());
            }
            spans.extend(vec![binding.into(), " edit latest queued".dim()]);
        }
        if spans.len() == 1 {
            return Vec::new();
        }

        adaptive_wrap_lines(
            std::iter::once(Line::from(spans)),
            RtOptions::new(width as usize).subsequent_indent(Line::from("  ".dim())),
        )
        .into_iter()
        .take(ITEM_ROW_CAP)
        .collect()
    }

    fn hidden_lines(&self, width: u16, hidden_items: usize) -> Vec<Line<'static>> {
        let mut spans = vec![format!("… {hidden_items} hidden").dim()];
        if let Some(binding) = self
            .interrupt_binding
            .filter(|_| !self.pending_steers.is_empty())
        {
            spans.extend(vec![" · ".dim(), binding.into(), " steers".dim()]);
        }
        if let Some(binding) = self
            .edit_binding
            .filter(|_| !self.queued_messages.is_empty())
        {
            spans.extend(vec![" · ".dim(), binding.into(), " edit queue".dim()]);
        }
        adaptive_wrap_lines(
            std::iter::once(Line::from(spans)),
            RtOptions::new(width as usize).subsequent_indent(Line::from("    ".dim())),
        )
        .into_iter()
        .take(ITEM_ROW_CAP)
        .collect()
    }

    fn item_lines(kind: PreviewKind, text: &str, width: u16) -> Vec<Line<'static>> {
        let (initial_indent, subsequent_indent) = match kind {
            PreviewKind::Rejected => (
                Line::from(vec!["  ! ".red(), "Retry: ".red()]),
                Line::from("           "),
            ),
            PreviewKind::Pending => (
                Line::from(vec!["  ↳ ".cyan(), "Steer: ".cyan()]),
                Line::from("           "),
            ),
            PreviewKind::Queued => (
                Line::from(vec!["  + ".dim(), "Queued: ".dim()]),
                Line::from("            "),
            ),
        };
        let styled_lines = text.split('\n').map(|line| {
            let line = if line.is_empty() { "blank line" } else { line };
            match kind {
                PreviewKind::Queued => Line::from(line.to_string().dim().italic()),
                PreviewKind::Rejected | PreviewKind::Pending => Line::from(line.to_string().dim()),
            }
        });
        let wrapped = adaptive_wrap_lines(
            styled_lines,
            RtOptions::new(width as usize)
                .initial_indent(initial_indent)
                .subsequent_indent(subsequent_indent.clone()),
        );
        let contains_clipped_line = wrapped.iter().any(|line| line.width() > width as usize);
        if wrapped.len() <= ITEM_ROW_CAP && !contains_clipped_line {
            return wrapped;
        }

        let first_line_is_clipped = wrapped
            .first()
            .is_some_and(|line| line.width() > width as usize);
        let hidden_rows = wrapped.len().saturating_sub(1) + usize::from(first_line_is_clipped);
        let mut lines = wrapped.into_iter().take(1).collect::<Vec<_>>();
        let disclosure = if first_line_is_clipped && hidden_rows == 1 {
            "… content clipped".to_string()
        } else {
            format!(
                "… +{hidden_rows} hidden line{}",
                if hidden_rows != 1 { "s" } else { "" }
            )
        };
        lines.push(Line::from(vec!["    ".into(), disclosure.dim()]));
        lines
    }

    fn preview_lines(&self, width: u16, row_limit: usize) -> Vec<Line<'static>> {
        if (self.pending_steers.is_empty()
            && self.rejected_steers.is_empty()
            && self.queued_messages.is_empty())
            || width < 4
            || row_limit == 0
        {
            return Vec::new();
        }

        let row_limit = row_limit.min(VISIBLE_ROW_CAP);
        let mut lines = self.title_lines(width);
        lines.truncate(row_limit);
        if lines.len() == row_limit {
            return lines;
        }

        let total_items =
            self.rejected_steers.len() + self.pending_steers.len() + self.queued_messages.len();
        let all_items = self
            .rejected_steers
            .iter()
            .map(|text| (PreviewKind::Rejected, text.as_str()))
            .chain(
                self.pending_steers
                    .iter()
                    .map(|text| (PreviewKind::Pending, text.as_str())),
            )
            .chain(
                self.queued_messages
                    .iter()
                    .map(|text| (PreviewKind::Queued, text.as_str())),
            );
        if row_limit <= 4
            && total_items > 1
            && let Some((kind, text)) = all_items.clone().next()
        {
            let item_lines = Self::item_lines(kind, text, width);
            let hidden_lines = self.hidden_lines(width, total_items - 1);
            let tail_rows = hidden_lines
                .len()
                .min(row_limit.saturating_sub(ITEM_ROW_CAP).max(1));
            let content_rows = row_limit - tail_rows;
            let item_is_truncated = item_lines.len() > content_rows;
            let mut lines = item_lines
                .into_iter()
                .take(content_rows)
                .collect::<Vec<_>>();
            if item_is_truncated && let Some(last_line) = lines.pop() {
                let mut last_line =
                    truncate_line_to_width(last_line, width.saturating_sub(1) as usize);
                last_line.spans.push("…".dim());
                lines.push(last_line);
            }
            lines.extend(hidden_lines.into_iter().take(tail_rows));
            return lines;
        }
        let action_lines = self.action_lines(width);
        let rows_after_title = row_limit - lines.len();
        let reserved_action_rows = if action_lines.len() <= rows_after_title.saturating_sub(1) {
            action_lines.len()
        } else {
            0
        };
        let item_budget = rows_after_title - reserved_action_rows;
        let mut used_item_rows = 0;
        let mut shown_items = 0;

        for (kind, text) in all_items {
            let item_lines = Self::item_lines(kind, text, width);
            let needs_hidden_count = shown_items + 1 < total_items;
            let required_rows = item_lines.len() + usize::from(needs_hidden_count);
            if used_item_rows + required_rows > item_budget {
                break;
            }
            used_item_rows += item_lines.len();
            shown_items += 1;
            lines.extend(item_lines);
        }

        let hidden_items = total_items - shown_items;
        if hidden_items > 0 && lines.len() < row_limit - reserved_action_rows {
            lines.extend(
                self.hidden_lines(width, hidden_items)
                    .into_iter()
                    .take(1 + reserved_action_rows),
            );
        } else {
            lines.extend(action_lines.into_iter().take(reserved_action_rows));
        }
        lines.truncate(row_limit);
        lines
    }
}

impl Renderable for PendingInputPreview {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let natural_lines = self.preview_lines(area.width, VISIBLE_ROW_CAP);
        let lines = if area.height as usize >= natural_lines.len() {
            natural_lines
        } else {
            self.preview_lines(area.width, area.height as usize)
        };
        for (row, line) in lines.iter().enumerate() {
            let row_area = Rect::new(area.x, area.y + row as u16, area.width, /*height*/ 1);
            line.render(row_area, buf);
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        u16::try_from(self.preview_lines(width, VISIBLE_ROW_CAP).len()).unwrap_or(u16::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    #[test]
    fn desired_height_empty() {
        let queue = PendingInputPreview::new();
        assert_eq!(queue.desired_height(/*width*/ 40), 0);
    }

    #[test]
    fn desired_height_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        assert_eq!(queue.desired_height(/*width*/ 40), 2);
    }

    #[test]
    fn render_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_message", format!("{buf:?}"));
    }

    #[test]
    fn render_one_message_with_shift_left_binding() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        queue.set_edit_binding(Some(key_hint::shift(KeyCode::Left).into()));
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_one_message_with_shift_left_binding",
            format!("{buf:?}")
        );
    }

    #[test]
    fn render_two_messages() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        queue
            .queued_messages
            .push("This is another message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_two_messages", format!("{buf:?}"));
    }

    #[test]
    fn render_more_than_three_messages() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        queue
            .queued_messages
            .push("This is another message".to_string());
        queue
            .queued_messages
            .push("This is a third message".to_string());
        queue
            .queued_messages
            .push("This is a fourth message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_more_than_three_messages", format!("{buf:?}"));
    }

    #[test]
    fn render_wrapped_message() {
        let mut queue = PendingInputPreview::new();
        queue
            .queued_messages
            .push("This is a longer message that should be wrapped".to_string());
        queue
            .queued_messages
            .push("This is another message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_wrapped_message", format!("{buf:?}"));
    }

    #[test]
    fn render_many_line_message() {
        let mut queue = PendingInputPreview::new();
        queue
            .queued_messages
            .push("This is\na message\nwith many\n\nlines".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_many_line_message", format!("{buf:?}"));
    }

    #[test]
    fn long_url_like_message_discloses_clipping_without_wrapping_the_token() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/session_id=abc123def456ghi789"
                .to_string(),
        );

        let width = 36;
        let height = queue.desired_height(width);
        assert_eq!(height, 4);

        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);

        let rendered_rows = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(
            rendered_rows
                .iter()
                .any(|row| row.contains("… content clipped")),
            "expected explicit clipping disclosure for URL-like token, got: {rendered_rows:?}"
        );
    }

    #[test]
    fn render_one_pending_steer() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push("Please continue.".to_string());
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_pending_steer", format!("{buf:?}"));
    }

    #[test]
    fn render_one_pending_steer_with_remapped_interrupt_binding() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push("Please continue.".to_string());
        queue.set_interrupt_binding(Some(key_hint::plain(KeyCode::F(12)).into()));
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_one_pending_steer_with_remapped_interrupt_binding",
            format!("{buf:?}")
        );
    }

    #[test]
    fn render_pending_steers_above_queued_messages() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push("Please continue.".to_string());
        queue
            .pending_steers
            .push("Check the last command output.".to_string());
        queue
            .rejected_steers
            .push("Rejected steer that will be retried.".to_string());
        queue
            .queued_messages
            .push("Queued follow-up question".to_string());
        let width = 52;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_pending_steers_above_queued_messages",
            format!("{buf:?}")
        );
    }

    #[test]
    fn render_multiline_pending_steer_uses_single_prefix_and_truncates() {
        let mut queue = PendingInputPreview::new();
        queue
            .pending_steers
            .push("First line\nSecond line\nThird line\n\nFourth line".to_string());
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_multiline_pending_steer_uses_single_prefix_and_truncates",
            format!("{buf:?}")
        );
    }

    #[test]
    fn shared_row_cap_and_hidden_count_cover_every_category() {
        let mut queue = PendingInputPreview::new();
        for index in 0..10 {
            queue.rejected_steers.push(format!("retry {index}"));
            queue.pending_steers.push(format!("steer {index}"));
            queue.queued_messages.push(format!("queued {index}"));
        }
        let width = 40;
        let height = queue.desired_height(width);
        assert!(height <= VISIBLE_ROW_CAP as u16);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        let rendered = format!("{buf:?}");
        assert!(rendered.contains("Retry: retry 0"));
        let shown_items = rendered.matches("Retry: retry").count();
        let hidden_items = 30 - shown_items;
        assert!(
            rendered.contains(&format!("… {hidden_items} hidden")),
            "visible and hidden inputs must conserve all 30 entries:\n{rendered}"
        );
    }

    #[test]
    fn render_responsive_mixed_input_matrix() {
        let mut queue = PendingInputPreview::new();
        queue
            .rejected_steers
            .push("Retry this required direction first.".to_string());
        queue
            .pending_steers
            .push("Inspect the current result before continuing.".to_string());
        queue.queued_messages.extend([
            "First queued follow-up stays first.".to_string(),
            "Unicode remains whole: 👩🏽‍💻 café 漢字.".to_string(),
            "https://example.test/releases/2026/artifacts/very-long-report-name".to_string(),
        ]);
        let snapshots = [24, 40, 80, 120]
            .into_iter()
            .map(|width| {
                let height = queue.desired_height(width);
                let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
                queue.render(Rect::new(0, 0, width, height), &mut buf);
                if width <= 40 {
                    let visible =
                        (0..width).any(|x| !buf[(x, height - 1)].symbol().trim().is_empty());
                    assert!(visible, "width {width} allocated a trailing blank row");
                }
                let rendered = format!("{buf:?}");
                assert!(rendered.contains("esc steers"));
                assert!(rendered.contains("edit") && rendered.contains("queue"));
                format!("{width} columns\n{rendered}")
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        assert_snapshot!("render_responsive_mixed_input_matrix", snapshots);
    }

    #[test]
    fn render_short_available_height_discloses_hidden_inputs() {
        let mut queue = PendingInputPreview::new();
        queue.rejected_steers.push("Required retry".to_string());
        queue.pending_steers.push("Active steer".to_string());
        queue.queued_messages.extend([
            "First queued".to_string(),
            "Second queued".to_string(),
            "Third queued".to_string(),
        ]);
        let snapshots = [2, 3, 4]
            .into_iter()
            .map(|height| {
                let mut buf = Buffer::empty(Rect::new(0, 0, /*width*/ 24, height));
                queue.render(Rect::new(0, 0, /*width*/ 24, height), &mut buf);
                let rendered = format!("{buf:?}");
                assert!(height != 2 || rendered.contains('…'));
                assert!(height != 4 || rendered.matches("esc steers").count() == 1);
                assert!(height != 4 || rendered.matches("edit queue").count() == 1);
                format!("{height} rows\n{rendered}")
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        assert_snapshot!("render_short_available_height", snapshots);
    }

    #[test]
    fn huge_multiline_input_has_bounded_item_rows_and_exact_line_count() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(
            (1..=20)
                .map(|line| format!("line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let lines = queue.preview_lines(/*width*/ 40, VISIBLE_ROW_CAP);
        let disclosure = lines[2]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(disclosure, "    … +19 hidden lines");
        let height = lines.len() as u16;
        let mut buf = Buffer::empty(Rect::new(0, 0, /*width*/ 40, height));
        queue.render(Rect::new(0, 0, /*width*/ 40, height), &mut buf);
        assert_snapshot!("render_huge_multiline_input", format!("{buf:?}"));
    }
}
