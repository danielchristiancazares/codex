use super::*;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;

fn handles() -> OutputHandles<1024> {
    OutputHandles {
        output_buffer: Arc::new(tokio::sync::Mutex::new(HeadTailBuffer::default())),
        output_notify: Arc::new(Notify::new()),
        output_closed: Arc::new(AtomicBool::new(false)),
        output_closed_notify: Arc::new(Notify::new()),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
    }
}

#[tokio::test]
async fn quiet_terminal_wait_preserves_output_until_collection() {
    let output = handles();
    let wait = output.wait_for_activity();
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(Duration::from_millis(30), &mut wait)
            .await
            .is_err()
    );
    output
        .output_buffer
        .lock()
        .await
        .push_chunk(b"new evidence");
    output.output_notify.notify_waiters();
    tokio::time::timeout(Duration::from_millis(50), &mut wait)
        .await
        .unwrap();
    assert_eq!(
        output.output_buffer.lock().await.total_bytes(),
        "new evidence".len()
    );
}

#[tokio::test]
async fn readiness_delivers_output_queued_before_subscription_and_process_exit() {
    let output = handles();
    output
        .output_buffer
        .lock()
        .await
        .push_chunk(b"already queued");
    output.output_notify.notify_waiters();
    tokio::time::timeout(Duration::from_millis(50), output.wait_for_activity())
        .await
        .unwrap();
    let exited = handles();
    exited.cancellation_token.cancel();
    tokio::time::timeout(Duration::from_millis(50), exited.wait_for_activity())
        .await
        .unwrap();
}

#[tokio::test]
async fn available_collection_drains_output_forwarded_just_after_process_exit() {
    let output = handles();
    output.cancellation_token.cancel();
    let delayed_output = output.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        delayed_output
            .output_buffer
            .lock()
            .await
            .push_chunk(b"final output");
        delayed_output.output_notify.notify_waiters();
        delayed_output.output_closed.store(true, Ordering::Release);
        delayed_output.output_closed_notify.notify_waiters();
    });

    let collected = UnifiedExecProcessManager::collect_output_until_deadline(
        &output,
        /*pause_state*/ None,
        Instant::now(),
    )
    .await;

    assert_eq!(collected.to_bytes_with_omission_marker(), b"final output");
}
