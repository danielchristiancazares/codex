use super::*;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsageInfo;

#[tokio::test]
async fn legacy_filtered_fork_recounts_parent_usage_for_the_child() {
    let mut harness = AgentControlHarness::new().await;
    let _ = harness.config.features.disable(Feature::MultiAgentV2);
    let parent = harness
        .manager
        .start_thread(StartThreadOptions {
            history_mode: Some(ThreadHistoryMode::Legacy),
            environments: Some(Vec::new()),
            ..StartThreadOptions::new(harness.config.clone())
        })
        .await
        .expect("legacy parent");
    let parent_usage = TokenUsage {
        input_tokens: 900_000,
        total_tokens: 900_000,
        ..TokenUsage::default()
    };
    parent
        .thread
        .session
        .persist_rollout_items(&[
            rollout_response_item(user_message("small retained context")),
            RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
                info: Some(TokenUsageInfo {
                    total_token_usage: parent_usage.clone(),
                    last_token_usage: parent_usage,
                    model_context_window: Some(1_000_000),
                }),
                rate_limits: None,
            })),
            rollout_response_item(spawn_agent_call("legacy-fork")),
        ])
        .await;
    let child_id = harness
        .spawn_anonymous_child(
            parent.thread_id,
            SpawnAgentOptions {
                fork_parent_spawn_call_id: Some("legacy-fork".to_string()),
                fork_mode: Some(SpawnAgentForkMode::FullHistory),
                ..Default::default()
            },
        )
        .await;
    let child = harness.manager.get_thread(child_id).await.expect("child");
    let usage = child.session.get_total_token_usage().await;
    assert!(
        usage > 0 && usage < 100_000,
        "child must use its retained context count, got {usage}"
    );
    child.flush_rollout().await.expect("flush child");
    let rollout = std::fs::read_to_string(child.rollout_path().expect("child rollout")).unwrap();
    assert!(!rollout.lines().filter_map(|line| codex_rollout::parse_rollout_line(line).ok()).any(|line| {
        matches!(line.item, RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent { info: Some(info), .. })) if info.last_token_usage.total_tokens == 900_000)
    }));
    child.submit(Op::Shutdown {}).await.unwrap();
    parent.thread.submit(Op::Shutdown {}).await.unwrap();
}
