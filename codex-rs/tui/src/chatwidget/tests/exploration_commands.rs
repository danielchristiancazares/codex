use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn inspection_batch_merges_with_surrounding_exploration() {
    for source in [
        ExecCommandSource::Agent,
        ExecCommandSource::UnifiedExecStartup,
    ] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.on_task_started();

        let first = begin_exec(&mut chat, "before", "cat before.rs");
        end_exec(&mut chat, first, "before\n", "", /*exit_code*/ 0);
        let batch = begin_exec_with_source(
            &mut chat,
            "batch",
            "rg -n 'root_turn|parent_turn' src\n\
             rg -n 'struct TurnStartOptions' src\n\
             sed -n '320,359p' src/turn_metadata.rs\n\
             sed -n '3375,3435p' src/guardian_tests.rs\n\
             rg -n 'required_entry_indices' src\n\
             git diff HEAD -- src/turn_metadata.rs",
            source,
        );
        end_exec(&mut chat, batch, "diff output\n", "", /*exit_code*/ 0);
        let last = begin_exec(&mut chat, "after", "cat after.rs");
        end_exec(&mut chat, last, "after\n", "", /*exit_code*/ 0);

        assert!(drain_insert_history_cells(&mut rx).is_empty());
        insta::allow_duplicates! {
                            insta::assert_snapshot!(active_blob(&chat), @r"
• Explored
  ├ Read before.rs, turn_metadata.rs, guardian_tests.rs, after.rs
  ├ Searched root_turn|parent_turn in src, struct TurnStartOptions in src,
  │          required_entry_indices in src
  └ Ran git diff HEAD -- src/turn_metadata.rs
");
                        }
        chat.flush_active_cell();
        let cells = drain_insert_history_cells(&mut rx);
        assert_eq!(cells.len(), 1);
        let transcript = lines_to_single_string(&cells[0].transcript_lines(/*width*/ 120));
        assert!(transcript.contains("git diff HEAD -- src/turn_metadata.rs"));
        assert!(transcript.contains("diff output"));
    }
}

#[tokio::test]
async fn inspection_completion_without_start_uses_exploration_summary() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "rg needle src\ngit diff HEAD".to_string(),
    ];
    let item = AppServerThreadItem::CommandExecution {
        id: "completed-batch".to_string(),
        command: crate::exec_command::escape_command(&command),
        cwd: chat.config.cwd.clone().into(),
        process_id: None,
        plugin_id: None,
        script_path: None,
        source: ExecCommandSource::Agent,
        status: AppServerCommandExecutionStatus::Completed,
        command_actions: codex_shell_command::parse_command::parse_command(&command)
            .into_iter()
            .map(|parsed| AppServerCommandAction::from_core_with_cwd(parsed, &chat.config.cwd))
            .collect(),
        aggregated_output: Some("diff output\n".to_string()),
        exit_code: Some(0),
        duration_ms: Some(5),
    };
    handle_exec_end(&mut chat, item);
    assert!(drain_insert_history_cells(&mut rx).is_empty());
    insta::assert_snapshot!(active_blob(&chat), @r"
• Explored
  ├ Searched needle in src
  └ Ran git diff HEAD
");
}

#[tokio::test]
async fn batch_with_execution_summarizes_commands_and_preserves_full_transcript() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();

    let call = begin_exec(&mut chat, "build", "rg needle src\ncargo test");
    end_exec(&mut chat, call, "test output\n", "", /*exit_code*/ 0);
    let cells = drain_insert_history_cells(&mut rx);
    assert_eq!(cells.len(), 1);
    let display = lines_to_single_string(&cells[0].display_lines(/*width*/ 120));
    insta::assert_snapshot!(display, @r"
• Ran 2 commands
  ├ Searched needle in src
  └ cargo test
");
    let transcript = lines_to_single_string(&cells[0].transcript_lines(/*width*/ 120));
    assert!(transcript.contains("rg needle src"));
    assert!(transcript.contains("cargo test"));
    assert!(transcript.contains("test output"));
}

#[tokio::test]
async fn replayed_exploration_preserves_live_grouping_and_command_boundaries() {
    for replay_kind in [
        ReplayKind::ResumeInitialMessages,
        ReplayKind::ThreadSnapshot,
    ] {
        for source in [
            ExecCommandSource::Agent,
            ExecCommandSource::UnifiedExecStartup,
        ] {
            let (mut live, mut live_rx, _live_ops) =
                make_chatwidget_manual(/*model_override*/ None).await;
            live.on_task_started();
            let mut items = Vec::new();
            for (id, command, output, code) in [
                ("failed", "rg needle missing", "missing: No such file\n", 2),
                ("read-first", "cat first.rs", "first\n", 0),
                ("read-second", "cat second.rs", "second\n", 0),
                ("search", "rg needle src", "src/lib.rs:needle\n", 0),
                ("list", "ls src", "lib.rs\n", 0),
                (
                    "glob-batch",
                    "sed -n '1,20p' src/first.rs\n\
                     rg -n 'fn on_task_complete' src/*.rs\n\
                     sed -n '20,40p' src/second.rs",
                    "fn on_task_complete() {}\n",
                    0,
                ),
                ("run", "printf checkpoint", "checkpoint\n", 0),
                (
                    "mixed-batch",
                    "rg -n 'fn .*command' src/helpers.rs\n\
                     sed -n '1,20p' src/streaming.rs\n\
                     command -v cargo-dylint dylint-link\n\
                     rustup toolchain list",
                    "cargo-dylint\ndylint-link\n1.98.1\n",
                    0,
                ),
                (
                    "failed-batch",
                    "rg needle src\nsed -n '1,20p' src/lib.rs\ncargo test",
                    "compilation failed\n",
                    1,
                ),
                ("read-after", "cat after.rs", "after\n", 0),
                ("read-last", "cat last.rs", "last\n", 0),
            ] {
                let mut item = begin_exec_with_source(&mut live, id, command, source);
                end_exec(&mut live, item.clone(), output, "", code);
                let AppServerThreadItem::CommandExecution {
                    status,
                    aggregated_output,
                    exit_code,
                    duration_ms,
                    ..
                } = &mut item
                else {
                    panic!("expected command execution item");
                };
                *status = if code == 0 {
                    AppServerCommandExecutionStatus::Completed
                } else {
                    AppServerCommandExecutionStatus::Failed
                };
                *aggregated_output = Some(output.to_string());
                *exit_code = Some(code);
                *duration_ms = Some(5);
                items.push(item);
            }
            handle_turn_completed(&mut live, "turn-1", /*duration_ms*/ None);
            let live_cells = drain_insert_history_cells(&mut live_rx)
                .into_iter()
                .filter(|cell| !cell.as_any().is::<history_cell::FinalMessageSeparator>())
                .collect::<Vec<_>>();

            let (mut replay, mut replay_rx, _replay_ops) =
                make_chatwidget_manual(/*model_override*/ None).await;
            replay.replay_thread_turns(
                vec![AppServerTurn {
                    items,
                    ..app_server_turn(
                        "turn-1",
                        AppServerTurnStatus::Completed,
                        /*duration_ms*/ None,
                        /*error*/ None,
                    )
                }],
                replay_kind,
            );
            let replay_cells = drain_insert_history_cells(&mut replay_rx);
            assert_eq!(
                replay_cells
                    .iter()
                    .map(|cell| cell.display_lines(/*width*/ 100))
                    .collect::<Vec<_>>(),
                live_cells
                    .iter()
                    .map(|cell| cell.display_lines(/*width*/ 100))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                replay_cells
                    .iter()
                    .map(|cell| cell.transcript_lines(/*width*/ 100))
                    .collect::<Vec<_>>(),
                live_cells
                    .iter()
                    .map(|cell| cell.transcript_lines(/*width*/ 100))
                    .collect::<Vec<_>>()
            );
            assert!(replay.running_commands.is_empty());
            assert!(replay.unified_exec_processes.is_empty());
            assert!(!replay.bottom_pane.is_task_running());
            insta::allow_duplicates! {
                insta::assert_snapshot!(
                    "replayed_exploration_grouping",
                    replay_cells
                        .iter()
                        .map(|cell| lines_to_single_string(&cell.display_lines(/*width*/ 100)))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
    }
}
