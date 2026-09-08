use super::*;
use crate::diff_model::FileChange;
use crate::style::StatusTone;
use crate::style::status_style;
use pretty_assertions::assert_eq;
use std::borrow::Cow;
use std::path::Path;

const VALID_DIFF: &str = "@@ -1 +1 @@\n-old\n+new\n";

#[test]
fn valid_unified_diff_is_borrowed_and_counted() {
    let prepared = PreparedUpdateDiff::new(VALID_DIFF, Some(Path::new("renamed/current-file.txt")));

    assert_eq!(prepared.mode(), UpdateDiffMode::Unified);
    assert_eq!(prepared.source(), VALID_DIFF);
    assert!(matches!(&prepared.source, Cow::Borrowed(_)));
    assert_eq!(prepared.line_counts(), (1, 1));
}

#[test]
fn matching_legacy_rename_trailer_is_stripped_after_validation() {
    for move_path in [
        Path::new("renamed/file.txt"),
        Path::new(r"C:\repo\renamed\file.txt"),
    ] {
        let legacy = format!("{VALID_DIFF}\n\nMoved to: {}", move_path.display());
        let prepared = PreparedUpdateDiff::new(&legacy, Some(move_path));

        assert_eq!(prepared.mode(), UpdateDiffMode::Unified);
        assert_eq!(prepared.source(), VALID_DIFF);
        assert!(matches!(&prepared.source, Cow::Owned(_)));
        assert_eq!(prepared.line_counts(), (1, 1));
    }
}

#[test]
fn nonmatching_legacy_rename_trailer_is_retained_for_fallback() {
    let legacy = format!("{VALID_DIFF}\n\nMoved to: other/file.txt");
    let prepared = PreparedUpdateDiff::new(&legacy, Some(Path::new("renamed/file.txt")));

    assert_eq!(prepared.mode(), UpdateDiffMode::RawFallback);
    assert_eq!(prepared.source(), legacy);
    assert_eq!(prepared.line_counts(), (1, 1));
}

#[test]
fn malformed_patch_uses_raw_counts_without_file_headers() {
    let malformed = "--- a/file.txt\n+++ b/file.txt\n-old\n+new\n";
    let prepared = PreparedUpdateDiff::new(malformed, /*move_path*/ None);

    assert_eq!(prepared.mode(), UpdateDiffMode::RawFallback);
    assert_eq!(prepared.source(), malformed);
    assert_eq!(prepared.line_counts(), (1, 1));
}

#[test]
fn nonempty_zero_hunk_payload_uses_visible_raw_fallback_counts() {
    let debug_approval_diff = "+test\n-test2";
    let prepared = PreparedUpdateDiff::new(debug_approval_diff, /*move_path*/ None);

    assert_eq!(prepared.mode(), UpdateDiffMode::RawFallback);
    assert_eq!(prepared.source(), debug_approval_diff);
    assert_eq!(prepared.line_counts(), (1, 1));
}

#[test]
fn raw_fallback_rendering_warns_and_keeps_changed_lines_visible() {
    let change = FileChange::Update {
        unified_diff: "+test\n-test2".to_string(),
        move_path: None,
    };
    let mut lines = Vec::new();

    assert!(!super::super::render_change(
        &change,
        &mut lines,
        /*width*/ 80,
        /*lang*/ None,
        usize::MAX,
    ));
    assert_eq!(
        lines
            .iter()
            .map(|line| {
                line.spans.iter().fold(String::new(), |mut text, span| {
                    text.push_str(span.content.as_ref());
                    text
                })
            })
            .collect::<Vec<_>>(),
        vec![
            RAW_FALLBACK_WARNING.to_string(),
            "+test".to_string(),
            "-test2".to_string(),
        ]
    );
    assert_eq!(lines[0].spans[0].style, status_style(StatusTone::Attention));
}
