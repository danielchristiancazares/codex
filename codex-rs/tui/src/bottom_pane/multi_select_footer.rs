use crossterm::event::KeyCode;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::key_hint;
use crate::key_hint::ShortcutHint;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::line_truncation::line_width;

pub(super) enum MultiSelectFooter {
    Custom(Line<'static>),
    Generated(Vec<Line<'static>>),
}

impl MultiSelectFooter {
    pub(super) fn new(
        instructions: Vec<Span<'static>>,
        keymap: &ListKeymap,
        ordering_enabled: bool,
    ) -> Self {
        if !instructions.is_empty() {
            return Self::Custom(Line::from(instructions));
        }

        Self::Generated(generated_candidates(keymap, ordering_enabled))
    }

    pub(super) fn line_for_width(&self, width: u16) -> Option<&Line<'static>> {
        match self {
            Self::Custom(line) => Some(line),
            Self::Generated(candidates) => candidates
                .iter()
                .find(|candidate| line_width(candidate) <= usize::from(width)),
        }
    }
}

fn generated_candidates(keymap: &ListKeymap, ordering_enabled: bool) -> Vec<Line<'static>> {
    let move_hints = if ordering_enabled {
        keymap
            .primary_hint(ListAction::MoveLeft)
            .zip(keymap.primary_hint(ListAction::MoveRight))
    } else {
        None
    };
    let accept = keymap.primary_hint(ListAction::Accept);
    let cancel = keymap.primary_hint(ListAction::Cancel);

    [
        verbose_candidate(move_hints, accept, cancel),
        compact_candidate(move_hints, accept, cancel),
        action_candidate(accept, cancel),
        keys_only_candidate(accept, cancel),
    ]
    .into_iter()
    .filter(|candidate| line_width(candidate) > 0)
    .collect()
}

fn verbose_candidate(
    move_hints: Option<(ShortcutHint, ShortcutHint)>,
    accept: Option<ShortcutHint>,
    cancel: Option<ShortcutHint>,
) -> Line<'static> {
    let mut spans = vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Char(' ')).into(),
        " to toggle".into(),
    ];
    if let Some((move_left, move_right)) = move_hints {
        push_separator(&mut spans);
        spans.extend([
            move_left.into(),
            "/".into(),
            move_right.into(),
            " to move".into(),
        ]);
    }
    push_labeled_hint(&mut spans, accept, " to confirm and close");
    push_labeled_hint(&mut spans, cancel, " to close");
    Line::from(spans)
}

fn compact_candidate(
    move_hints: Option<(ShortcutHint, ShortcutHint)>,
    accept: Option<ShortcutHint>,
    cancel: Option<ShortcutHint>,
) -> Line<'static> {
    let mut spans = Vec::new();
    push_labeled_hint(&mut spans, accept, " confirm");
    push_labeled_hint(&mut spans, cancel, " close");
    push_separator(&mut spans);
    spans.extend([key_hint::plain(KeyCode::Char(' ')).into(), " toggle".into()]);
    if let Some((move_left, move_right)) = move_hints {
        push_separator(&mut spans);
        spans.extend([
            move_left.into(),
            "/".into(),
            move_right.into(),
            " move".into(),
        ]);
    }
    Line::from(spans)
}

fn action_candidate(accept: Option<ShortcutHint>, cancel: Option<ShortcutHint>) -> Line<'static> {
    let mut spans = Vec::new();
    push_labeled_hint(&mut spans, accept, " confirm");
    push_labeled_hint(&mut spans, cancel, " close");
    Line::from(spans)
}

fn keys_only_candidate(
    accept: Option<ShortcutHint>,
    cancel: Option<ShortcutHint>,
) -> Line<'static> {
    let mut spans = Vec::new();
    push_labeled_hint(&mut spans, accept, "");
    push_labeled_hint(&mut spans, cancel, "");
    Line::from(spans)
}

fn push_labeled_hint(
    spans: &mut Vec<Span<'static>>,
    hint: Option<ShortcutHint>,
    label: &'static str,
) {
    let Some(hint) = hint else {
        return;
    };
    push_separator(spans);
    spans.extend([hint.into(), label.into()]);
}

fn push_separator(spans: &mut Vec<Span<'static>>) {
    if !spans.is_empty() {
        spans.push("; ".into());
    }
}

#[cfg(test)]
#[path = "multi_select_footer_tests.rs"]
mod tests;
