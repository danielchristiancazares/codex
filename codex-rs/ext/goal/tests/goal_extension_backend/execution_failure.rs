use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn plan_turn_resets_the_execution_failure_streak() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;
    tool_by_name(&harness.tools(), "create_goal")
        .handle(tool_call(
            "create_goal",
            "create",
            json!({"objective": "recover execution"}),
        ))
        .await?;
    for turn in 1..=6 {
        let turn_id = format!("turn-{turn}");
        if turn > 1 {
            let mode = if turn == 3 {
                ModeKind::Plan
            } else {
                ModeKind::Default
            };
            harness
                .start_turn_with_mode(&turn_id, mode, &TokenUsage::default())
                .await;
        }
        harness
            .notify_tool_finish_with_outcome(
                &turn_id,
                &format!("exec-{turn}"),
                "exec",
                ToolCallOutcome::Failed {
                    handler_executed: true,
                },
            )
            .await;
        harness.stop_turn(&turn_id).await;
        let goal = runtime
            .thread_goals()
            .get_thread_goal(thread_id)
            .await?
            .expect("active goal");
        assert_eq!(
            goal.status,
            if turn == 6 {
                codex_state::ThreadGoalStatus::Blocked
            } else {
                codex_state::ThreadGoalStatus::Active
            }
        );
    }
    Ok(())
}

#[tokio::test]
async fn execution_block_storage_failure_reports_once_and_retires_the_turn() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;
    tool_by_name(&harness.tools(), "create_goal")
        .handle(tool_call(
            "create_goal",
            "create",
            json!({"objective": "recover execution"}),
        ))
        .await?;
    for turn in 1..=3 {
        let turn_id = format!("turn-{turn}");
        if turn > 1 {
            harness.start_turn(&turn_id, &TokenUsage::default()).await;
        }
        harness
            .notify_tool_finish_with_outcome(
                &turn_id,
                &format!("exec-{turn}"),
                "exec",
                ToolCallOutcome::Failed {
                    handler_executed: true,
                },
            )
            .await;
        if turn < 3 {
            harness.stop_turn(&turn_id).await;
        }
    }
    harness
        .record_token_usage(
            "turn-3",
            &TokenUsage {
                input_tokens: 10,
                total_tokens: 10,
                ..Default::default()
            },
        )
        .await;
    harness.sink.clear();
    runtime.close().await;
    harness.stop_turn("turn-3").await;
    harness.stop_turn("turn-3").await;
    let errors = harness
        .sink
        .events()
        .iter()
        .filter_map(|event| {
            if let EventMsg::Error(error) = &event.msg {
                Some((event.id.clone(), error.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let [(event_id, error)] = errors.as_slice() else {
        anyhow::bail!(
            "expected one execution-block accounting error, got {}",
            errors.len()
        );
    };
    assert_eq!(event_id, "turn-3:goal-accounting-error");
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::Other));
    assert!(
        error
            .message
            .starts_with("failed to persist goal execution block; automatic continuation stopped:")
    );
    assert_eq!(harness.sink.goal_events(), Vec::<CapturedGoalEvent>::new());
    Ok(())
}

#[tokio::test]
async fn stale_failure_evidence_cannot_charge_or_block_a_replacement_goal() -> anyhow::Result<()> {
    let runtime = test_runtime().await?;
    let thread_id = test_thread_id()?;
    seed_thread_metadata(runtime.as_ref(), thread_id).await?;
    let harness = GoalExtensionHarness::new(runtime.clone(), thread_id).await?;
    harness.start_turn("turn-1", &TokenUsage::default()).await;
    tool_by_name(&harness.tools(), "create_goal")
        .handle(tool_call(
            "create_goal",
            "create",
            json!({"objective": "old objective"}),
        ))
        .await?;
    for turn in 1..=3 {
        let turn_id = format!("turn-{turn}");
        if turn > 1 {
            harness.start_turn(&turn_id, &TokenUsage::default()).await;
        }
        harness
            .notify_tool_finish_with_outcome(
                &turn_id,
                &format!("exec-{turn}"),
                "exec",
                ToolCallOutcome::Failed {
                    handler_executed: true,
                },
            )
            .await;
        if turn < 3 {
            harness.stop_turn(&turn_id).await;
        }
    }
    harness
        .record_token_usage(
            "turn-3",
            &TokenUsage {
                input_tokens: 10,
                total_tokens: 10,
                ..Default::default()
            },
        )
        .await;
    // Exercise the persistence-to-runtime-projection gap with old accounting evidence.
    let replacement = runtime
        .thread_goals()
        .replace_thread_goal(
            thread_id,
            "replacement objective",
            codex_state::ThreadGoalStatus::Active,
            /*token_budget*/ None,
        )
        .await?;
    harness.sink.clear();
    harness.stop_turn("turn-3").await;
    assert_eq!(
        runtime.thread_goals().get_thread_goal(thread_id).await?,
        Some(replacement)
    );
    assert_eq!(harness.sink.goal_events(), Vec::<CapturedGoalEvent>::new());
    Ok(())
}
