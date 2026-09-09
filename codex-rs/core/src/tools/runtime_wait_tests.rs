use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn coordination_wait_leaves_exclusive_tool_admission_open() {
    let lock = RwLock::new(());
    let _waiting = ExecutionScope::Coordination
        .acquire(&lock, ParallelExecution::Exclusive)
        .await;
    let exclusive = tokio::time::timeout(
        Duration::from_millis(50),
        ExecutionScope::Serialized.acquire(&lock, ParallelExecution::Exclusive),
    )
    .await
    .expect("a dependency must be able to execute while a coordination wait is pending");
    drop(exclusive);
}

#[tokio::test]
async fn omitted_wait_deadline_stays_pending_and_explicit_deadline_finishes() {
    assert!(
        tokio::time::timeout(
            Duration::from_millis(30),
            WaitPolicy::default().expired(1, 10),
        )
        .await
        .is_err()
    );
    let explicit: WaitPolicy = serde_json::from_str("5").unwrap();
    tokio::time::timeout(Duration::from_millis(50), explicit.expired(1, 10))
        .await
        .expect("explicit deadline should finish");
}

#[tokio::test]
async fn subscription_preserves_queued_activity_and_burst_notifications() {
    assert_eq!(
        InputWakeup::DeliverQueued(InputQueueActivity::Steer)
            .wait()
            .await
            .unwrap(),
        InputQueueActivity::Steer,
    );
    let (sender, receiver) = watch::channel(InputQueueActivity::Mailbox);
    sender.send_replace(InputQueueActivity::Mailbox);
    sender.send_replace(InputQueueActivity::Steer);
    assert_eq!(
        InputWakeup::AwaitQueued(receiver).wait().await.unwrap(),
        InputQueueActivity::Steer,
    );
}
