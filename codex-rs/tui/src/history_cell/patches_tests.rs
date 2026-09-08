//! Coverage for patch-apply-failure and image-tool transcript cells.

use super::*;
use pretty_assertions::assert_eq;

fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn patch_apply_failure_uses_shared_red_failure_tone() {
    let cell = new_patch_apply_failure("error: hunk #1 failed to apply\n".to_string());
    let lines = cell.display_lines(/*width*/ 80);

    let title = lines
        .first()
        .expect("failure cell should render a title line");
    assert_eq!(
        title
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        "✗ Failed to apply patch",
    );
    assert!(
        title
            .spans
            .iter()
            .all(|span| span.style.fg == Some(Color::Red)),
        "failure title should use the shared red failure tone (matching approvals and image \
         generation failures), not the Codex magenta brand accent: {title:?}",
    );

    insta::assert_snapshot!(render_lines(&lines).join("\n"));
}

#[test]
fn patch_apply_failure_without_stderr_shows_only_title() {
    let cell = new_patch_apply_failure(String::new());
    let lines = cell.display_lines(/*width*/ 80);
    assert_eq!(
        render_lines(&lines),
        vec!["✗ Failed to apply patch".to_string()],
    );
}

#[test]
fn consecutive_patches_combine_file_counts_and_preserve_each_diff() {
    let first = HashMap::from([(
        PathBuf::from("src/example.rs"),
        FileChange::Update {
            unified_diff: diffy::create_patch("old\n", "first\n").to_string(),
            move_path: None,
        },
    )]);
    let second = HashMap::from([
        (
            PathBuf::from("src/example.rs"),
            FileChange::Update {
                unified_diff: diffy::create_patch("first\n", "second\nthird\n").to_string(),
                move_path: None,
            },
        ),
        (
            PathBuf::from("src/added.rs"),
            FileChange::Add {
                content: "one\ntwo\n".to_string(),
            },
        ),
    ]);
    let cwd = Path::new("/project");
    let reasoning =
        new_reasoning_summary_block(vec!["Review the remaining edits.".to_string()], cwd);
    let mut expected_transcript = create_diff_summary(&first, cwd, /*wrap_cols*/ 100);
    expected_transcript.push(Line::default());
    expected_transcript.extend(reasoning.transcript_lines(/*width*/ 100));
    expected_transcript.push(Line::default());
    expected_transcript.extend(create_diff_summary(&second, cwd, /*wrap_cols*/ 100));
    let mut group = new_patch_event(first, cwd);
    group.append_transcript(reasoning);
    group.append_changes(second);

    assert_eq!(group.transcript_lines(/*width*/ 100), expected_transcript);
    insta::assert_snapshot!(
        render_lines(&group.display_lines(/*width*/ 100)).join("\n"),
        @"
    • Edited 2 files (+5 -2)
      ├ A src/added.rs (+2 -0)
      └ M src/example.rs (+3 -2)
    "
    );
}
