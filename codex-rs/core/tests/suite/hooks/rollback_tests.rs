use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn rollback_discards_async_hook_context_from_the_removed_turn() -> Result<()> {
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_assistant_message("m1", "done"), ev_completed("r1")]),
            sse(vec![
                ev_assistant_message("m2", "continued"),
                ev_completed("r2"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_async_user_prompt_submit_hook(home, /*gated*/ true)
                .expect("write asynchronous hook");
        })
        .with_config(trust_discovered_hooks)
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("discard this turn").await?;
    fs_wait::wait_for_path_exists(
        test.codex_home_path()
            .join("async_user_prompt_submit_started"),
        Duration::from_secs(5),
    )
    .await?;

    test.codex
        .submit(Op::ThreadRollback { num_turns: 1 })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ThreadRolledBack(_))
    })
    .await;
    fs::write(
        test.codex_home_path()
            .join("async_user_prompt_submit_release"),
        "ready",
    )?;
    test.submit_turn("continue after rollback").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        !requests[1]
            .message_input_texts("developer")
            .iter()
            .any(|text| { text.contains("async context for discard this turn") })
    );
    assert_eq!(
        requests[1]
            .message_input_texts("user")
            .last()
            .map(String::as_str),
        Some("continue after rollback")
    );
    Ok(())
}

#[tokio::test]
async fn rollback_persists_surviving_async_hook_context_after_the_cut() -> Result<()> {
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_assistant_message("m1", "first done"), ev_completed("r1")]),
            sse(vec![ev_assistant_message("m2", "second done"), ev_completed("r2")]),
            sse(vec![ev_assistant_message("m3", "resumed"), ev_completed("r3")]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_async_user_prompt_submit_hook(home, /*gated*/ true)
                .expect("write asynchronous hook");
        })
        .with_config(trust_discovered_hooks);
    let initial = builder.build_with_auto_env(&server).await?;
    initial.submit_turn("retain this hook context").await?;
    fs_wait::wait_for_path_exists(
        initial
            .codex_home_path()
            .join("async_user_prompt_submit_started"),
        Duration::from_secs(5),
    )
    .await?;
    initial.submit_turn("discard the latest turn").await?;
    fs::write(
        initial
            .codex_home_path()
            .join("async_user_prompt_submit_release"),
        "ready",
    )?;
    let finished_path = initial
        .codex_home_path()
        .join("async_user_prompt_submit_finished");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if fs::read_to_string(&finished_path)
                .is_ok_and(|finished| finished.lines().count() >= 2)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;

    initial
        .codex
        .submit(Op::ThreadRollback { num_turns: 1 })
        .await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::ThreadRolledBack(_))
    })
    .await;

    let mut resume_builder = test_codex().with_config(trust_discovered_hooks);
    let resumed = resume_builder.restart(&server, &initial).await?;
    resumed.submit_turn("continue after resume").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let resumed_context = requests[2].message_input_texts("developer");
    assert!(
        resumed_context
            .iter()
            .any(|text| text.contains("async context for retain this hook context"))
    );
    assert!(
        resumed_context
            .iter()
            .all(|text| !text.contains("async context for discard the latest turn"))
    );
    Ok(())
}
