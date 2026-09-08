use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Stylize;

#[test]
fn compact_workspace_paths_keep_the_leaf_styles_and_other_segments() {
    let mut snapshots = Vec::new();
    for (path, wide_path) in [
        ("/work/projects/atlas", "/work/projects/atlas"),
        (r"C:\work\projects\atlas", r"C:\work\projects\atlas"),
        (
            r"~\AppData\Local\Temp\review\atlas",
            r"…\Local\Temp\review\atlas",
        ),
        ("/work/日本語/atlas", "/work/日本語/atlas"),
    ] {
        let original = Line::from(vec![
            "GPT 5.6 Sol · High reasoning · ".into(),
            path.to_string().bold(),
        ]);
        assert_eq!(
            truncate_status_line(original.clone(), /*max_width*/ 120),
            Line::from(vec![
                original.spans[0].clone(),
                wide_path.to_string().bold()
            ])
        );
        let compact = truncate_status_line(original.clone(), /*max_width*/ 42);
        assert_eq!(compact.spans[0], original.spans[0]);
        assert_eq!(compact.spans[1].style, original.spans[1].style);
        assert!(compact.width() <= 42);
        assert!(compact.to_string().ends_with("atlas"));
        snapshots.push(compact.to_string());
    }
    insta::assert_snapshot!(
        "workspace_leaf_under_footer_truncation",
        snapshots.join("\n")
    );
}

#[test]
fn narrow_paths_keep_a_grapheme_safe_identifying_suffix() {
    let line = Line::from(vec!["Model · ".into(), "/projects/cafe\u{301}".bold()]);
    assert_eq!(
        truncate_status_line(line, /*max_width*/ 12),
        Line::from(vec!["Model · ".into(), "cafe\u{301}".bold()])
    );
    assert_eq!(
        truncate_status_line(
            Line::from(vec![
                "GPT 5.6 Sol · Default reasoning · ".into(),
                r"C:\work\atlas".bold()
            ]),
            /*max_width*/ 40
        ),
        Line::from(vec![
            "GPT 5.6 Sol · Default reasoning · ".into(),
            "atlas".bold()
        ])
    );
}

#[test]
fn narrow_active_footer_keeps_workspace_and_title_progress() {
    let original = Line::from(vec![
        "GPT 5.6 Sol · High reasoning".cyan(),
        " · ".dim(),
        r"~\AppData\Local\Temp\review\atlas".green().bold(),
        " · ".dim(),
        "renaming... ◐".magenta(),
    ]);
    let mut snapshots = Vec::new();
    for width in [46, 47, 48, 78, 118] {
        let compact = truncate_status_line(original.clone(), width);
        assert!(compact.width() <= width);
        assert!(compact.to_string().contains("atlas"));
        assert!(compact.to_string().contains("renaming... ◐"));
        assert!(compact.to_string().starts_with("GPT 5.6 Sol"));
        assert_eq!(
            compact
                .spans
                .iter()
                .map(|span| span.style)
                .collect::<Vec<_>>(),
            original
                .spans
                .iter()
                .map(|span| span.style)
                .collect::<Vec<_>>()
        );
        snapshots.push(format!("{width}: {compact}"));
    }
    insta::assert_snapshot!(
        "active_footer_workspace_and_title_progress",
        snapshots.join("\n")
    );
}
