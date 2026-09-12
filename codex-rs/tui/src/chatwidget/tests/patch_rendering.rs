use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn consecutive_edits_render_individual_diffs_live_and_on_replay() {
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
    let cells = drain_insert_history_cells(&mut rx);
    assert_eq!(
        cells
            .iter()
            .filter(|cell| cell.as_any().is::<history_cell::PatchHistoryCell>())
            .count(),
        3,
    );
    assert!(chat.transcript.active_cell.is_none());
    let rendered = cells
        .iter()
        .map(|cell| {
            (
                cell.display_lines(/*width*/ 100),
                cell.transcript_lines(/*width*/ 100),
            )
        })
        .collect::<Vec<_>>();
    insta::assert_snapshot!(
        "consecutive_inline_diffs",
        rendered
            .iter()
            .filter(|(display, _)| !display.is_empty())
            .map(|(display, _)| lines_to_single_string(display))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    insta::assert_snapshot!(
        "consecutive_diff_transcript",
        rendered
            .iter()
            .map(|(_, transcript)| lines_to_single_string(transcript))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    handle_turn_completed(&mut chat, "turn-1", /*duration_ms*/ None);
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
        assert_eq!(
            replayed
                .iter()
                .map(|cell| {
                    (
                        cell.display_lines(/*width*/ 100),
                        cell.transcript_lines(/*width*/ 100),
                    )
                })
                .collect::<Vec<_>>(),
            rendered,
        );
        assert!(replay.transcript.active_cell.is_none());
    }
}

#[tokio::test]
async fn inline_diffs_preserve_command_and_failure_order() {
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
        "inline_diffs_with_command_and_failure",
        cells
            .iter()
            .map(|cell| lines_to_single_string(&cell.display_lines(/*width*/ 100)))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    assert!(chat.transcript.active_cell.is_none());
}
