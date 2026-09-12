//! Coverage for inline patch diffs and patch-apply failures.

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
fn patch_history_renders_inline_diff_and_full_transcript() {
    let changes = HashMap::from([
        (
            PathBuf::from("src/example.rs"),
            FileChange::Update {
                unified_diff: diffy::create_patch("old\n", "new\n").to_string(),
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
    let cell = new_patch_event(changes, Path::new("/project"));
    let display = cell.display_lines(/*width*/ 80);

    assert_eq!(cell.transcript_lines(/*width*/ 80), display);
    assert_eq!(cell.raw_lines(), plain_lines(display.clone()));
    insta::assert_snapshot!(render_lines(&display).join("\n"));
}
