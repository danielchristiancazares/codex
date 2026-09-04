//! Shared popup-related constants for bottom pane widgets.

use ratatui::style::Stylize;
use ratatui::text::Line;

use crate::key_hint;
use crate::key_hint::ShortcutHint;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crossterm::event::KeyCode;

/// Maximum number of rows any popup should attempt to display.
/// Keep this consistent across all popups for a uniform feel.
pub(crate) const MAX_POPUP_ROWS: usize = 8;

/// Standard footer hint text used by popups.
pub(crate) fn standard_popup_hint_line() -> Line<'static> {
    key_hint::action_hint_line(
        "",
        [
            (key_hint::plain(KeyCode::Enter), "confirm"),
            (key_hint::plain(KeyCode::Esc), "back"),
        ],
    )
}

pub(crate) fn standard_popup_hint_line_for_keymap(list_keymap: &ListKeymap) -> Line<'static> {
    accept_cancel_hint_line(
        list_keymap.primary_hint(ListAction::Accept),
        "confirm",
        list_keymap.primary_hint(ListAction::Cancel),
        "back",
    )
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
            format!(" {accept_label}").dim(),
            " · ".dim(),
            cancel.into(),
            format!(" {cancel_label}").dim(),
        ]),
        (Some(accept), None) => Line::from(vec![accept.into(), format!(" {accept_label}").dim()]),
        (None, Some(cancel)) => Line::from(vec![cancel.into(), format!(" {cancel_label}").dim()]),
        (None, None) => Line::from(""),
    }
}
