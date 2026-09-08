//! Combine per-file counts across consecutive patches while their full diffs stay separate.

use super::DiffSummaryDetail;
use super::Row;
use super::collect_rows;
use super::preview;
use super::render_changes_block;
use crate::diff_model::FileChange;
use ratatui::text::Line;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::btree_map::Entry;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn create_grouped_diff_file_summary<'a>(
    patches: impl IntoIterator<Item = &'a HashMap<PathBuf, FileChange>>,
    cwd: &Path,
    width: usize,
) -> Vec<Line<'static>> {
    let mut rows: BTreeMap<&Path, Row<'_>> = BTreeMap::new();
    for changes in patches {
        for row in collect_rows(changes) {
            match rows.entry(row.path) {
                Entry::Vacant(entry) => {
                    entry.insert(row);
                }
                Entry::Occupied(mut entry) => {
                    let previous = entry.get_mut();
                    previous.added += row.added;
                    previous.removed += row.removed;
                    previous.change_count += row.change_count;
                    previous.change = row.change;
                    previous.move_path = row.move_path.or(previous.move_path);
                }
            }
        }
    }
    render_changes_block(
        rows.into_values().collect(),
        width.max(1),
        cwd,
        DiffSummaryDetail::Files,
        preview::PREVIEW_ROWS,
    )
}
