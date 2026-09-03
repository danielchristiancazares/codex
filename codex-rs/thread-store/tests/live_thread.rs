use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::TokenCountEvent;
use codex_rollout::RolloutItem;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::InMemoryThreadStore;
use codex_thread_store::LiveThread;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::LocalThreadStoreConfig;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_thread_store::ThreadStoreError;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test]
async fn append_items_preserves_mixed_batch_persistence_policy() {
    let thread_id = ThreadId::new();
    let live_thread = LiveThread::create(
        Arc::new(InMemoryThreadStore::default()),
        create_thread_params(thread_id),
    )
    .await
    .expect("create live thread");
    let initial_items = live_thread
        .load_history(/*include_archived*/ false)
        .await
        .expect("load initial history")
        .items;
    let durable_item = RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
        info: None,
        rate_limits: None,
        rollout_budget: None,
    }));

    live_thread
        .append_items(&[transient_delta(thread_id), durable_item.clone()])
        .await
        .expect("append mixed batch");

    let mut expected_items = initial_items;
    expected_items.push(durable_item);
    assert_eq!(
        serde_json::to_value(
            live_thread
                .load_history(/*include_archived*/ false)
                .await
                .expect("load appended history")
                .items
        )
        .expect("serialize actual history"),
        serde_json::to_value(expected_items).expect("serialize expected history")
    );
}

#[tokio::test]
async fn transient_append_after_discard_preserves_local_lifecycle_error() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        LocalThreadStoreConfig {
            codex_home: home.path().to_path_buf(),
            sqlite: codex_state::SqliteConfig::new_for_testing(home.path().abs()),
            default_model_provider_id: "test-provider".to_string(),
        },
        /*state_db*/ None,
    ));
    let thread_id = ThreadId::new();
    let mut params = create_thread_params(thread_id);
    params.metadata.cwd = Some(home.path().to_path_buf());
    let live_thread = LiveThread::create(store, params)
        .await
        .expect("create live thread");
    live_thread.discard().await.expect("discard live thread");

    assert!(matches!(
        live_thread
            .append_items(&[transient_delta(thread_id)])
            .await,
        Err(ThreadStoreError::ThreadNotFound {
            thread_id: missing_thread_id
        }) if missing_thread_id == thread_id
    ));
}

fn create_thread_params(thread_id: ThreadId) -> CreateThreadParams {
    CreateThreadParams {
        session_id: thread_id.into(),
        thread_id,
        extra_config: None,
        forked_from_id: None,
        parent_thread_id: None,
        source: SessionSource::Exec,
        thread_source: None,
        originator: "test_originator".to_string(),
        base_instructions: BaseInstructions::default(),
        dynamic_tools: Vec::new(),
        selected_capability_roots: Vec::new(),
        multi_agent_version: None,
        history_mode: ThreadHistoryMode::Legacy,
        history_base: None,
        subagent_history_start_ordinal: None,
        initial_window_id: "window-test".to_string(),
        metadata: ThreadPersistenceMetadata {
            cwd: None,
            model_provider: "test-provider".to_string(),
            memory_mode: ThreadMemoryMode::Enabled,
        },
    }
}

fn transient_delta(thread_id: ThreadId) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::AgentMessageContentDelta(
        AgentMessageContentDeltaEvent {
            thread_id: thread_id.to_string(),
            turn_id: "turn-test".to_string(),
            item_id: "item-test".to_string(),
            delta: "delta".to_string(),
        },
    ))
}
