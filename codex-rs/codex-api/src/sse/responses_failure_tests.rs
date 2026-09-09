use super::*;
use bytes::Bytes;
use codex_client::TransportError;
use futures::stream;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn failed_response_finishes_without_waiting_for_eof() {
    let failed = json!({
        "type": "response.failed",
        "response": {"error": {"code": "insufficient_quota", "message": "Quota exceeded"}}
    });
    let bytes = Bytes::from(format!("data: {failed}\n\n"));
    let stream = stream::iter([Ok(bytes)]).chain(stream::pending());
    let (tx, mut rx) = mpsc::channel(8);
    let task = tokio::spawn(process_sse(
        Box::pin(stream),
        tx,
        Duration::from_millis(20),
        /*telemetry*/ None,
    ));

    assert!(matches!(
        rx.recv().await,
        Some(Err(ApiError::QuotaExceeded))
    ));
    assert!(rx.recv().await.is_none());
    task.await.expect("SSE reader should exit");
}

#[tokio::test]
async fn failed_response_preserves_retry_delay_before_transport_failure() {
    let message = "Rate limit reached. Please try again in 11.054s.";
    let failed = json!({
        "type": "response.failed",
        "response": {"error": {"code": "rate_limit_exceeded", "message": message}}
    });
    let bytes = Bytes::from(format!("data: {failed}\n\n"));
    let stream = stream::iter([
        Ok(bytes),
        Err(TransportError::Network("connection lost".to_string())),
    ]);
    let (tx, mut rx) = mpsc::channel(8);
    let task = tokio::spawn(process_sse(
        Box::pin(stream),
        tx,
        Duration::from_secs(1),
        /*telemetry*/ None,
    ));

    match rx.recv().await {
        Some(Err(ApiError::RateLimitExceeded {
            message: actual,
            delay,
        })) => assert_eq!(
            (actual.as_str(), delay),
            (message, Some(Duration::from_millis(11054)))
        ),
        result => panic!("expected the original rate-limit failure, got {result:?}"),
    }
    assert!(rx.recv().await.is_none());
    task.await.expect("SSE reader should exit");
}
