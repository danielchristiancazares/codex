use super::*;
use crate::exec_cell::CommandOutput;
use crate::exec_cell::new_active_exec_command;
use crate::history_cell::HistoryCell;
use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use pretty_assertions::assert_eq;
use std::time::Duration;

fn render_line_text(line: &Line<'static>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn exploration_cell(commands: &[&str]) -> ExecCell {
    let mut calls = commands
        .iter()
        .enumerate()
        .map(|(index, command)| {
            let command = vec!["bash".into(), "-lc".into(), (*command).to_string()];
            ExecCall {
                call_id: format!("call-{index}"),
                parsed: codex_shell_command::parse_command::parse_command(&command),
                command,
                output: Some(CommandOutput::default()),
                source: ExecCommandSource::Agent,
                start_time: None,
                duration: Some(Duration::from_millis(5)),
                interaction_input: None,
            }
        })
        .collect_vec();
    assert!(calls.iter().all(ExecCell::is_exploring_call));
    let first = calls.remove(/*index*/ 0);
    let mut cell = ExecCell::new(first, /*animations_enabled*/ false);
    cell.calls.extend(calls);
    cell
}

#[test]
fn groups_interleaved_exploration_by_action() {
    let cell = exploration_cell(&[
        "cat lib.rs && cat endpoint.rs",
        "rg 'CopilotModelProvider::new|fn.*provider|struct.*Provider|codex_home|store_mode' registry.rs",
        "cat types.rs",
        "cat storage_tests.rs",
        "rg 'pub.*fn new|pub.*fn.*store|codex_home|auth_credentials_store_mode' manager.rs",
        "rg 'codex_model_provider::|ModelProviderFactory|build_model_provider|create_model_provider' mod.rs",
        "cat Cargo.toml",
        "ls .github",
        "cat justfile",
        "cat BUILD.bazel",
        "cat provider.rs && cat manager.rs && cat endpoint.rs && cat auth_keyring.rs",
        r"rg 'create_model_provider\(' codex-rs",
    ]);
    assert_eq!(
        cell.display_lines(u16::MAX)
            .iter()
            .map(render_line_text)
            .collect_vec(),
        vec![
            "• Explored",
            "  ├ Read lib.rs, endpoint.rs, types.rs, storage_tests.rs, Cargo.toml, justfile, BUILD.bazel, provider.rs, manager.rs, endpoint.rs, auth_keyring.rs",
            r"  ├ Searched CopilotModelProvider::new|fn.*provider|struct.*Provider|codex_home|store_mode in registry.rs, pub.*fn new|pub.*fn.*store|codex_home|auth_credentials_store_mode in manager.rs, codex_model_provider::|ModelProviderFactory|build_model_provider|create_model_provider in mod.rs, create_model_provider\( in codex-rs",
            "  └ Listed .github",
        ]
    );

    let mut responsive = Vec::new();
    for width in [40, 80, 120] {
        let lines = cell.display_lines(width);
        assert!(lines.iter().all(|line| line.width() <= usize::from(width)));
        responsive.push((width, lines.iter().map(render_line_text).collect_vec()));
    }
    insta::assert_debug_snapshot!("interleaved_exploration_grouped_by_action", responsive);
}

#[test]
fn groups_actions_within_mixed_calls_in_first_seen_order() {
    let cell = exploration_cell(&[
        "ls src && cat lib.rs && rg needle src && cat types.rs",
        "rg other tests && ls tests && cat registry.rs",
    ]);
    let rendered = cell
        .display_lines(/*width*/ 120)
        .iter()
        .map(render_line_text)
        .join("\n");
    insta::assert_snapshot!(rendered, @r"
    • Explored
      ├ Listed src, tests
      ├ Read lib.rs, types.rs, registry.rs
      └ Searched needle in src, other in tests
    ");
}

#[test]
fn grouped_actions_stay_active_until_all_their_calls_finish() {
    let mut cell = exploration_cell(&[
        "cat lib.rs",
        "rg needle src",
        "ls src",
        "cat endpoint.rs",
        "rg other tests",
        "ls tests",
    ]);
    for call in &mut cell.calls[3..] {
        call.duration = None;
    }
    let active = cell.display_lines(/*width*/ 80);

    for call_id in ["call-3", "call-4"] {
        assert!(cell.complete_call(call_id, CommandOutput::default(), Duration::from_millis(5),));
    }
    let partially_completed = cell.display_lines(/*width*/ 80);

    assert!(cell.complete_call("call-5", CommandOutput::default(), Duration::from_millis(5),));
    let completed = cell.display_lines(/*width*/ 80);

    insta::assert_debug_snapshot!(
        "grouped_exploration_lifecycle",
        (active, partially_completed, completed)
    );
}

#[test]
fn grouped_actions_preserve_command_fallbacks_and_query_paths() {
    let mut cell = exploration_cell(&["rg needle", "ls", "rg other tests", "ls .github"]);
    cell.calls[0].parsed = vec![ParsedCommand::Search {
        cmd: "rg --files -g '*.rs' src".into(),
        query: None,
        path: Some("src".into()),
    }];
    let rendered = cell
        .display_lines(/*width*/ 120)
        .iter()
        .map(render_line_text)
        .join("\n");
    insta::assert_snapshot!(rendered, @r"
    • Explored
      ├ Searched rg --files -g '*.rs' src, other in tests
      └ Listed ls, .github
    ");
}

#[test]
fn powershell_skill_read_snapshot() {
    let command = vec![
        "powershell.exe".to_string(),
        "-Command".to_string(),
        r"Get-Content C:\skills\demo\SKILL.md".to_string(),
    ];
    let parsed = codex_shell_command::parse_command::parse_command(&command);
    let cell = new_active_exec_command(
        "call-id".to_string(),
        command,
        parsed,
        ExecCommandSource::Agent,
        /*interaction_input*/ None,
        /*animations_enabled*/ false,
    );
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(render_line_text)
        .join("\n");

    insta::assert_snapshot!(rendered, @r"
    • Exploring
      └ Read SKILL.md
    ");
}

#[test]
fn exploring_preview_truncates_long_url_like_search_query_without_wrapping() {
    let url_like = "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/with/a/very/long/path";
    let call = ExecCall {
        call_id: "call-id".to_string(),
        command: vec!["bash".into(), "-lc".into(), "rg foo".into()],
        parsed: vec![ParsedCommand::Search {
            cmd: format!("rg {url_like}"),
            query: Some(url_like.to_string()),
            path: None,
        }],
        output: None,
        source: ExecCommandSource::Agent,
        start_time: None,
        duration: None,
        interaction_input: None,
    };

    let cell = ExecCell::new(call, /*animations_enabled*/ false);
    let rendered = cell
        .display_lines(/*width*/ 36)
        .iter()
        .map(render_line_text)
        .collect_vec();

    assert_eq!(
        rendered,
        vec![
            "• Exploring".to_string(),
            "  └ Search example.test/api/v1/proj…".to_string(),
        ]
    );
}
