use super::*;

#[tokio::test]
async fn raw_output_mode_requeues_a_held_table_tail_and_restarts_commits() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.handle_streaming_delta("| Key | Value |\n| --- | --- |\n| alpha | beta |\n".to_string());
    assert!(chat.active_cell_is_stream_tail());
    assert!(chat.stream_controllers_idle());
    while rx.try_recv().is_ok() {}

    chat.set_raw_output_mode(/*enabled*/ true);

    assert!(!chat.active_cell_is_stream_tail());
    assert!(!chat.stream_controllers_idle());
    let mut restarted_commits = false;
    while let Ok(event) = rx.try_recv() {
        restarted_commits |= matches!(event, AppEvent::StartCommitAnimation);
    }
    assert!(
        restarted_commits,
        "raw mode should restart commits for lines released from table holdback",
    );

    chat.on_commit_tick();
    let history = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(
        history.contains("| Key | Value |"),
        "expected the released raw table header to render, got {history:?}",
    );
    assert_chatwidget_snapshot!("raw_output_mode_releases_held_stream_table", history);
}

#[tokio::test]
async fn owned_agent_finalization_requires_reflow_after_deferred_mode_change() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let cwd = chat.config.cwd.to_path_buf();
    let mut controller = crate::streaming::controller::StreamController::new(
        Some(/*width*/ 24),
        cwd.as_path(),
        HistoryRenderMode::Rich,
    );
    controller.push("**bold response** with deferred rows.\n");
    controller.clear_queue();
    controller.set_render_mode(HistoryRenderMode::Raw);
    chat.stream_controller = Some(controller);
    while rx.try_recv().is_ok() {}

    chat.flush_answer_stream_with_separator();

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AppEvent::InsertHistoryCell(_))),
        "owned canonical replay must not insert stale deferred rows first"
    );
    let consolidation = events.into_iter().find_map(|event| match event {
        AppEvent::ConsolidateAgentMessage {
            scrollback_reflow,
            deferred_history_cell,
            ..
        } => Some((scrollback_reflow, deferred_history_cell)),
        _ => None,
    });
    let (scrollback_reflow, deferred_history_cell) =
        consolidation.expect("agent consolidation event");
    assert_eq!(
        scrollback_reflow,
        crate::app_event::ConsolidationScrollbackReflow::Required
    );
    assert!(deferred_history_cell.is_some());
}

#[tokio::test]
async fn inline_agent_finalization_inserts_unobserved_rows_before_consolidation() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.transcript_replay_policy =
        crate::transcript_reflow::TranscriptReplayPolicy::InlinePreserveScrollback;
    chat.handle_streaming_delta("inline response rows\n".to_string());
    while rx.try_recv().is_ok() {}

    chat.flush_answer_stream_with_separator();

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let insert_index = events
        .iter()
        .position(|event| matches!(event, AppEvent::InsertHistoryCell(_)))
        .expect("unobserved inline cell insert");
    let consolidate_index = events
        .iter()
        .position(|event| matches!(event, AppEvent::ConsolidateAgentMessage { .. }))
        .expect("inline consolidation");
    assert!(insert_index < consolidate_index);
    match &events[consolidate_index] {
        AppEvent::ConsolidateAgentMessage {
            scrollback_reflow,
            deferred_history_cell,
            ..
        } => {
            assert_eq!(
                *scrollback_reflow,
                crate::app_event::ConsolidationScrollbackReflow::InlinePreserve(
                    crate::app_event::InlineCanonicalCorrection::None,
                )
            );
            assert!(deferred_history_cell.is_none());
        }
        _ => unreachable!("located consolidation event"),
    }
}

#[tokio::test]
async fn inline_plan_finalization_inserts_unobserved_rows_before_consolidation() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.transcript_replay_policy =
        crate::transcript_reflow::TranscriptReplayPolicy::InlinePreserveScrollback;
    let source = "1. Inline plan row\n";
    let mut controller = crate::streaming::controller::PlanStreamController::new(
        Some(/*width*/ 40),
        chat.config.cwd.as_path(),
        HistoryRenderMode::Rich,
    );
    controller.push(source);
    chat.plan_stream_controller = Some(controller);
    chat.transcript.plan_delta_buffer = source.to_string();
    while rx.try_recv().is_ok() {}

    chat.on_plan_item_completed(String::new());

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let insert_index = events
        .iter()
        .position(|event| {
            matches!(event, AppEvent::InsertHistoryCell(cell) if cell.as_any().is::<history_cell::ProposedPlanStreamCell>())
        })
        .expect("unobserved inline plan insert");
    let consolidate_index = events
        .iter()
        .position(|event| matches!(event, AppEvent::ConsolidateProposedPlan(_)))
        .expect("inline plan consolidation");
    assert!(insert_index < consolidate_index);
}

#[tokio::test]
async fn inline_active_raw_toggle_stays_pending_until_consolidation_boundary() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.transcript_replay_policy =
        crate::transcript_reflow::TranscriptReplayPolicy::InlinePreserveScrollback;
    chat.handle_streaming_delta("**current response**\n".to_string());
    while rx.try_recv().is_ok() {}

    chat.acknowledge_raw_output_mode_request(/*enabled*/ true);
    chat.defer_raw_output_mode_until_stream_boundary(/*enabled*/ true);

    assert!(!chat.raw_output_mode());
    assert!(chat.requested_raw_output_mode());
    assert_eq!(
        chat.status_line_value_for_item(crate::bottom_pane::StatusLineItem::RawOutput),
        None
    );
    let acknowledgement_count = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .filter(|text| text.contains("Raw output will apply after the current response."))
        .count();
    assert_eq!(acknowledgement_count, 1);

    chat.flush_answer_stream_with_separator();
    assert!(
        chat.take_pending_raw_output_mode_after_stream().is_none(),
        "pending mode must wait for app consolidation"
    );
    chat.note_stream_consolidation_completed();
    let enabled = chat
        .take_pending_raw_output_mode_after_stream()
        .expect("pending raw mode after consolidation");
    chat.set_raw_output_mode(enabled);
    assert!(chat.raw_output_mode());

    chat.handle_streaming_delta("**next response**\n".to_string());
    chat.on_commit_tick();
    let rendered = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("**next response**"),
        "next stream should start in raw mode: {rendered:?}"
    );
}
