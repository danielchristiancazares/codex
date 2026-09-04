use std::path::PathBuf;

use codex_file_search::FileMatch;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use crate::render::Insets;
use crate::render::RectExt;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;

/// Visual state for the file-search popup.
pub(crate) struct FileSearchPopup {
    /// Query corresponding to the `matches` currently shown.
    display_query: String,
    /// Latest query typed by the user. May differ from `display_query` when
    /// a search is still in-flight.
    pending_query: String,
    /// When `true` we are still waiting for results for `pending_query`.
    waiting: bool,
    /// Cached matches; paths relative to the search dir.
    matches: Vec<FileMatch>,
    /// Shared selection/scroll state.
    state: ScrollState,
}

impl FileSearchPopup {
    pub(crate) fn new() -> Self {
        Self {
            display_query: String::new(),
            pending_query: String::new(),
            waiting: true,
            matches: Vec::new(),
            state: ScrollState::new(),
        }
    }

    /// Update the query and reset state to *waiting*.
    pub(crate) fn set_query(&mut self, query: &str) {
        if query == self.pending_query {
            return;
        }

        self.pending_query.clear();
        self.pending_query.push_str(query);

        self.waiting = true; // waiting for new results
    }

    /// Put the popup into an "idle" state used for an empty query (just "@").
    /// Shows a hint instead of matches until the user types more characters.
    pub(crate) fn set_empty_prompt(&mut self) {
        self.display_query.clear();
        self.pending_query.clear();
        self.waiting = false;
        self.matches.clear();
        // Reset selection/scroll state when showing the empty prompt.
        self.state.reset();
    }

    /// Replace matches when a `FileSearchResult` arrives.
    /// Replace matches. Only applied when `query` matches `pending_query`.
    pub(crate) fn set_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        if query != self.pending_query {
            return; // stale
        }

        self.display_query = query.to_string();
        self.matches = matches.into_iter().take(MAX_POPUP_ROWS).collect();
        self.waiting = false;
        let len = self.matches.len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move selection cursor up.
    pub(crate) fn move_up(&mut self) {
        let len = self.matches.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    /// Move selection cursor down.
    pub(crate) fn move_down(&mut self) {
        let len = self.matches.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub(crate) fn selected_match(&self) -> Option<&PathBuf> {
        self.state
            .selected_idx
            .and_then(|idx| self.matches.get(idx))
            .map(|file_match| &file_match.path)
    }

    pub(crate) fn calculate_required_height(&self) -> u16 {
        // Row count depends on whether we already have matches. If no matches
        // yet (e.g. initial search or query with no results) reserve a single
        // row so the popup is still visible. When matches are present we show
        // up to MAX_RESULTS regardless of the waiting flag so the list
        // remains stable while a newer search is in-flight.

        self.matches.len().clamp(1, MAX_POPUP_ROWS) as u16
    }
}

impl WidgetRef for &FileSearchPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Convert matches to GenericDisplayRow, translating indices to usize at the UI boundary.
        let rows_all: Vec<GenericDisplayRow> = if self.matches.is_empty() {
            Vec::new()
        } else {
            self.matches
                .iter()
                .enumerate()
                .map(|(index, m)| GenericDisplayRow {
                    name: m.path.to_string_lossy().to_string(),
                    name_prefix_spans: if self.state.selected_idx == Some(index) {
                        vec!["› ".into()]
                    } else {
                        vec!["  ".into()]
                    },
                    match_indices: m
                        .indices
                        .as_ref()
                        .map(|v| v.iter().map(|&i| i as usize).collect()),
                    display_shortcut: None,
                    description: None,
                    category_tag: None,
                    wrap_indent: None,
                    is_disabled: false,
                    disabled_reason: None,
                })
                .collect()
        };

        let empty_message = if self.waiting {
            "Searching files…"
        } else if self.pending_query.is_empty() {
            "Type to search files"
        } else {
            "No matching files"
        };

        render_rows(
            area.inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
            buf,
            &rows_all,
            &self.state,
            MAX_POPUP_ROWS,
            empty_message,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_file_search::MatchType;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::WidgetRef;

    fn file_match(index: usize) -> FileMatch {
        FileMatch {
            score: index as u32,
            path: PathBuf::from(format!("file_{index:02}.rs")),
            match_type: MatchType::File,
            root: PathBuf::from("/tmp/repo"),
            indices: None,
        }
    }

    fn render_popup(popup: &FileSearchPopup, width: u16) -> String {
        let area = Rect::new(0, 0, width, popup.calculate_required_height());
        let mut buf = Buffer::empty(area);
        popup.render_ref(area, &mut buf);
        (0..area.height)
            .map(|row| {
                (0..area.width)
                    .map(|column| buf[(column, row)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn set_matches_keeps_only_the_first_page_of_results() {
        let mut popup = FileSearchPopup::new();
        popup.set_query("file");
        popup.set_matches("file", (0..(MAX_POPUP_ROWS + 2)).map(file_match).collect());

        assert_eq!(
            popup.matches,
            (0..MAX_POPUP_ROWS).map(file_match).collect::<Vec<_>>()
        );
        assert_eq!(popup.calculate_required_height(), MAX_POPUP_ROWS as u16);
    }

    #[test]
    fn file_search_popup_states_snapshot() {
        let mut searching = FileSearchPopup::new();
        searching.set_query("src");

        let mut empty = FileSearchPopup::new();
        empty.set_empty_prompt();

        let mut no_matches = FileSearchPopup::new();
        no_matches.set_query("src");
        no_matches.set_matches("src", Vec::new());

        let mut matches = FileSearchPopup::new();
        matches.set_query("file");
        matches.set_matches("file", vec![file_match(1), file_match(2)]);
        matches.move_down();

        insta::assert_snapshot!(
            "file_search_popup_states",
            format!(
                "searching:\n{}\n\nempty:\n{}\n\nno matches:\n{}\n\nselected:\n{}\n\nnarrow:\n{}",
                render_popup(&searching, /*width*/ 64),
                render_popup(&empty, /*width*/ 64),
                render_popup(&no_matches, /*width*/ 64),
                render_popup(&matches, /*width*/ 64),
                render_popup(&matches, /*width*/ 28),
            )
        );
    }
}
