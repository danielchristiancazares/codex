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
