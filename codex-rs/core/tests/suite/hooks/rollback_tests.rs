use super::*;

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
