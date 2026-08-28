use super::*;
use crate::history_cell::HistoryCell;
use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use itertools::Itertools;
use pretty_assertions::assert_eq;
use ratatui::prelude::Line;
use std::time::Instant;

fn render_line_text(line: &Line<'static>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn roster_call(id: &str, command: &str, state: CompactCallState) -> ExecCall {
    let duration =
        (state != CompactCallState::Active).then_some(std::time::Duration::from_millis(5));
    let exit_code = if state == CompactCallState::Failed {
        7
    } else {
        0
    };
    ExecCall {
        call_id: id.to_string(),
        command: vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            command.to_string(),
        ],
        parsed: Vec::new(),
        output: Some(CommandOutput::new(
            exit_code,
            format!("stdout: {id}\n\x1b[31mstderr: {id}\x1b[0m"),
        )),
        source: ExecCommandSource::Agent,
        start_time: (state == CompactCallState::Active).then(Instant::now),
        duration,
        interaction_input: None,
    }
}

fn full_roster_fixture() -> ExecCell {
    let specs = [
        ("failed-url", CompactCallState::Failed),
        ("done-one", CompactCallState::Succeeded),
        ("failed-unicode", CompactCallState::Failed),
        ("running-three", CompactCallState::Active),
        ("failed-long", CompactCallState::Failed),
        ("done-five", CompactCallState::Succeeded),
        ("failed-six", CompactCallState::Failed),
        ("done-seven", CompactCallState::Succeeded),
        ("failed-eight", CompactCallState::Failed),
        ("running-nine", CompactCallState::Active),
        ("done-ten", CompactCallState::Succeeded),
        ("current-running", CompactCallState::Active),
    ];
    let mut calls = specs
        .map(|(id, state)| roster_call(id, &format!("printf {id}"), state))
        .into_iter()
        .collect_vec();
    calls[0].command[3] =
        "curl https://example.test/api/v1/projects/alpha/releases/2026/artifacts/report.json"
            .into();
    calls[2].command[3] = r"Get-Content C:\工作区\项目\非常长的目录\诊断.log".into();
    calls[4].command[3] =
        "python scripts/check.py --workspace a-very-long-workspace-name --mode exhaustive".into();
    calls[11].command[3] =
        "current-selected-command cargo test -p codex-tui exec_cell::render".into();
    let first = calls.remove(0);
    let mut cell = ExecCell::new(first, /*animations_enabled*/ false);
    cell.calls.extend(calls);
    cell
}

#[test]
fn full_command_roster_is_responsive_and_transcript_complete() {
    let cell = full_roster_fixture();
    let mut responsive = Vec::new();
    for width in [24, 40, 80, 120] {
        let lines = cell.display_lines(width);
        assert_eq!(lines.len(), 14);
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        let rendered = lines.iter().map(render_line_text).join("\n");
        assert!(!rendered.contains("hidden") && !rendered.contains("all in transcript"));
        assert!(!rendered.contains('✓') && !rendered.contains('✗') && !rendered.contains('●'));
        responsive.push((width, lines.iter().map(render_line_text).collect_vec()));
    }

    let wide = &responsive[3].1;
    assert!(wide[1].contains("curl https://example.test"));
    assert!(wide[2].contains("printf done-one"));
    assert!(wide[3].contains(r"Get-Content C:\工作区\项目"));
    assert!(wide[12].contains("current-selected-command"));
    assert!(wide[13].contains("Error: stderr: failed-eight"));

    let transcript = cell
        .transcript_lines(/*width*/ 80)
        .iter()
        .map(render_line_text)
        .join("\n");
    assert!(transcript.contains("printf failed-eight"));
    assert!(transcript.contains("stdout: failed-eight"));
    assert!(transcript.contains("stderr: failed-eight"));

    let narrow_before = responsive[0].1.clone();
    insta::assert_debug_snapshot!("full_command_roster_responsive", responsive);
    let narrow_after = cell
        .display_lines(/*width*/ 24)
        .iter()
        .map(render_line_text)
        .collect_vec();
    assert_eq!(narrow_after, narrow_before);
}

#[test]
fn semantic_exploration_group_remains_semantic_beyond_four_calls() {
    let mut calls = (1..=5)
        .map(|index| ExecCall {
            call_id: format!("search-{index}"),
            command: vec!["rg".to_string(), format!("query-{index}")],
            parsed: vec![ParsedCommand::Search {
                cmd: format!("rg query-{index}"),
                query: Some(format!("query-{index}")),
                path: Some("codex-rs/tui".to_string()),
            }],
            output: Some(CommandOutput::new(/*exit_code*/ 0, String::new())),
            source: ExecCommandSource::Agent,
            start_time: None,
            duration: Some(std::time::Duration::from_millis(5)),
            interaction_input: None,
        })
        .collect_vec();
    let first = calls.remove(0);
    let mut cell = ExecCell::new(first, /*animations_enabled*/ false);
    cell.calls.extend(calls);

    let lines = cell.display_lines(/*width*/ 80);
    assert_eq!(
        lines.iter().map(render_line_text).collect_vec(),
        vec![
            "• Explored",
            "  ├ Searched query-1 in codex-rs/tui",
            "  ├ Searched query-2 in codex-rs/tui",
            "  ├ Searched query-3 in codex-rs/tui",
            "  ├ Searched query-4 in codex-rs/tui",
            "  └ Searched query-5 in codex-rs/tui",
        ]
    );
    insta::assert_debug_snapshot!("semantic_exploration_group_unbounded", lines);
}

#[test]
fn terminal_failure_output_replaces_running_state_before_duration_arrives() {
    let call = roster_call("lifecycle", "lifecycle-command", CompactCallState::Active);
    let mut cell = ExecCell::new(call, /*animations_enabled*/ false);
    let before_display = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(render_line_text)
        .join("\n");
    let before_transcript = cell
        .transcript_lines(/*width*/ 80)
        .iter()
        .map(render_line_text)
        .join("\n");
    cell.calls[0]
        .output
        .as_mut()
        .expect("live output")
        .exit_code = 1;
    let failed_display = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(render_line_text)
        .join("\n");
    let failed_transcript = cell
        .transcript_lines(/*width*/ 80)
        .iter()
        .map(render_line_text)
        .join("\n");

    assert_eq!(
        (
            before_display.contains("Running 1 command"),
            before_display.contains("└ lifecycle-command"),
            before_transcript.contains("✗ (1)"),
            failed_display.contains("Running"),
            failed_display.contains("Ran 1 command · 1 failed"),
            failed_display.contains("Error: stderr: lifecycle"),
            failed_transcript.contains("✗ (1)"),
        ),
        (true, true, false, false, true, true, true)
    );
}
