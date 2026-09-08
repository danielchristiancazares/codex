use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn consecutive_edits_share_one_active_summary_until_turn_completion() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    let mut items = Vec::new();
    for (id, path, before, after) in [
        ("first", "src/first.rs", "old\n", "first\n"),
        ("second", "src/second.rs", "old\n", "second\nthird\n"),
        ("third", "src/first.rs", "first\n", "updated\n"),
    ] {
        let diff = diffy::create_patch(before, after).to_string();
        let changes = HashMap::from([(
            PathBuf::from(path),
            FileChange::Update {
                unified_diff: diff.clone(),
                move_path: None,
            },
        )]);
        handle_patch_apply_begin(&mut chat, id, "turn-1", changes.clone());
        handle_patch_apply_end(
            &mut chat,
            id,
            "turn-1",
            changes,
            AppServerPatchApplyStatus::Completed,
        );
        items.push(AppServerThreadItem::FileChange {
            id: id.to_string(),
            changes: vec![FileUpdateChange {
                path: path.to_string(),
                kind: PatchChangeKind::Update { move_path: None },
                diff,
            }],
            status: AppServerPatchApplyStatus::Completed,
        });
        if id == "first" {
            let reasoning = "Review the remaining edits.".to_string();
            chat.on_agent_reasoning_delta(reasoning.clone());
            chat.on_agent_reasoning_final();
            items.push(AppServerThreadItem::Reasoning {
                id: "reasoning".to_string(),
                summary: vec![reasoning],
                content: Vec::new(),
            });
        }
    }
    assert!(drain_insert_history_cells(&mut rx).is_empty());
    insta::assert_snapshot!(active_blob(&chat), @"
    • Edited 2 files (+4 -3)
      ├ M src/first.rs (+2 -2)
      └ M src/second.rs (+2 -1)
    ");

    handle_turn_completed(&mut chat, "turn-1", /*duration_ms*/ None);
    let cells = drain_insert_history_cells(&mut rx);
    let patches = cells
        .iter()
        .filter(|cell| cell.as_any().is::<history_cell::PatchHistoryCell>())
        .collect::<Vec<_>>();
    assert_eq!(patches.len(), 1);
    assert!(chat.transcript.active_cell.is_none());
    let transcript = lines_to_single_string(&patches[0].transcript_lines(/*width*/ 100));
    assert_eq!(transcript.matches("• Edited").count(), 3);
    assert!(transcript.contains("updated"));
    assert!(transcript.contains("third"));

    for replay_kind in [
        ReplayKind::ResumeInitialMessages,
        ReplayKind::ThreadSnapshot,
    ] {
        let (mut replay, mut replay_rx, _ops) =
            make_chatwidget_manual(/*model_override*/ None).await;
        replay.replay_thread_turns(
            vec![AppServerTurn {
                items: items.clone(),
                ..app_server_turn(
                    "turn-1",
                    AppServerTurnStatus::Completed,
                    /*duration_ms*/ None,
                    /*error*/ None,
                )
            }],
            replay_kind,
        );
        let replayed = drain_insert_history_cells(&mut replay_rx);
        assert_eq!(replayed.len(), 1);
        assert_eq!(
            replayed[0].display_lines(/*width*/ 100),
            patches[0].display_lines(/*width*/ 100),
        );
        assert_eq!(
            replayed[0].transcript_lines(/*width*/ 100),
            patches[0].transcript_lines(/*width*/ 100),
        );
        assert!(replay.transcript.active_cell.is_none());
    }
}

#[tokio::test]
async fn command_and_failure_boundaries_end_edit_groups() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_task_started();
    let changes = HashMap::from([(
        PathBuf::from("src/example.rs"),
        FileChange::Update {
            unified_diff: diffy::create_patch("old\n", "new\n").to_string(),
            move_path: None,
        },
    )]);
    handle_patch_apply_begin(&mut chat, "first", "turn-1", changes.clone());
    handle_patch_apply_end(
        &mut chat,
        "first",
        "turn-1",
        changes.clone(),
        AppServerPatchApplyStatus::Completed,
    );
    let call = begin_exec(&mut chat, "check", "printf checkpoint");
    end_exec(&mut chat, call, "checkpoint\n", "", /*exit_code*/ 0);
    handle_patch_apply_begin(&mut chat, "failed", "turn-1", changes.clone());
    handle_patch_apply_end(
        &mut chat,
        "failed",
        "turn-1",
        changes.clone(),
        AppServerPatchApplyStatus::Failed,
    );
    handle_patch_apply_begin(&mut chat, "last", "turn-1", changes);
    let cells = drain_insert_history_cells(&mut rx);
    insta::assert_snapshot!(
        cells
            .iter()
            .map(|cell| lines_to_single_string(&cell.display_lines(/*width*/ 100)))
            .collect::<Vec<_>>()
            .join("\n"),
        @"
    • Edited src/example.rs (+1 -1)

    • Ran 1 command
      └ printf checkpoint

    • Edited src/example.rs (+1 -1)

    ✗ Failed to apply patch
    "
    );
    assert_eq!(active_blob(&chat), "• Edited src/example.rs (+1 -1)\n");
}
