//! Shared popup-related constants for bottom pane widgets.

use crate::style::secondary_style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::key_hint::ShortcutHint;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::keymap::RuntimeKeymap;

/// Maximum number of rows any popup should attempt to display.
/// Keep this consistent across all popups for a uniform feel.
pub(crate) const MAX_POPUP_ROWS: usize = 8;

/// Standard footer hint text used by popups.
pub(crate) fn standard_popup_hint_line() -> Line<'static> {
    standard_popup_hint_line_for_keymap(&RuntimeKeymap::defaults().list)
}

pub(crate) fn standard_popup_hint_line_for_keymap(list_keymap: &ListKeymap) -> Line<'static> {
    let mut spans = Vec::new();
    if let (Some(up), Some(down)) = (
        list_keymap.primary_hint(ListAction::MoveUp),
        list_keymap.primary_hint(ListAction::MoveDown),
    ) {
        spans.push(up.into());
        spans.push(Span::styled("/", secondary_style()));
        spans.push(down.into());
        spans.push(Span::styled(" choose · ", secondary_style()));
    }
    spans.extend(
        accept_cancel_hint_line(
            list_keymap.primary_hint(ListAction::Accept),
            "confirm",
            list_keymap.primary_hint(ListAction::Cancel),
            "back",
        )
        .spans,
    );
    Line::from(spans)
}

pub(crate) fn accept_cancel_hint_line(
    accept: Option<ShortcutHint>,
    accept_label: &'static str,
    cancel: Option<ShortcutHint>,
    cancel_label: &'static str,
) -> Line<'static> {
    match (accept, cancel) {
        (Some(accept), Some(cancel)) => Line::from(vec![
            accept.into(),
            Span::styled(format!(" {accept_label}"), secondary_style()),
            " · ".dim(),
            cancel.into(),
            Span::styled(format!(" {cancel_label}"), secondary_style()),
        ]),
        (Some(accept), None) => Line::from(vec![
            accept.into(),
            Span::styled(format!(" {accept_label}"), secondary_style()),
        ]),
        (None, Some(cancel)) => Line::from(vec![
            cancel.into(),
            Span::styled(format!(" {cancel_label}"), secondary_style()),
        ]),
        (None, None) => Line::from(""),
    }
}
