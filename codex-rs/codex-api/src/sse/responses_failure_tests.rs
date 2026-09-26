use super::*;
use bytes::Bytes;
use codex_client::TransportError;
use futures::stream;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(start_paused = true)]
async fn failed_response_finishes_without_waiting_for_eof() {
    let failed = json!({
        "type": "response.failed",
        "response": {"error": {"code": "insufficient_quota", "message": "Quota exceeded"}}
    });
    let bytes = Bytes::from(format!("data: {failed}\n\n"));
    let stream = stream::iter([Ok(bytes)]).chain(stream::pending());
    let (tx, mut rx) = mpsc::channel(/*buffer*/ 8);
    let task = tokio::spawn(process_sse(
        Box::pin(stream),
        tx,
        Duration::from_secs(30),
        /*telemetry*/ None,
    ));

    assert!(matches!(
        rx.recv().await,
        Some(Err(ApiError::QuotaExceeded))
    ));
    assert!(rx.recv().await.is_none());
    task.await.expect("SSE reader should exit");
}

#[tokio::test(start_paused = true)]
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
    let (tx, mut rx) = mpsc::channel(/*buffer*/ 8);
    let task = tokio::spawn(process_sse(
        Box::pin(stream),
        tx,
        Duration::from_secs(1),
        /*telemetry*/ None,
    ));

    match rx.recv().await {
        Some(Err(ApiError::RateLimitExceeded {
            message: actual,
            retry_after,
        })) => assert_eq!(
            (
                actual.as_str(),
                retry_after.expect("server retry deadline").remaining_delay()
            ),
            (message, Duration::from_millis(11054))
        ),
        result => panic!("expected the original rate-limit failure, got {result:?}"),
    }
    assert!(rx.recv().await.is_none());
    task.await.expect("SSE reader should exit");
}

#[tokio::test(start_paused = true)]
async fn incomplete_response_preserves_usage_and_is_not_retryable() {
    let incomplete = json!({
        "type": "response.incomplete",
        "response": {
            "id": "resp-incomplete",
            "incomplete_details": {"reason": "max_output_tokens"},
            "usage": {
                "input_tokens": 120,
                "input_tokens_details": {"cached_tokens": 30, "cache_write_tokens": 2},
                "output_tokens": 50,
                "output_tokens_details": {"reasoning_tokens": 40},
                "total_tokens": 170
            }
        }
    });
    let bytes = Bytes::from(format!("data: {incomplete}\n\n"));
    let stream = stream::iter([Ok(bytes)]).chain(stream::pending());
    let (tx, mut rx) = mpsc::channel(/*buffer*/ 8);
    let task = tokio::spawn(process_sse(
        Box::pin(stream),
        tx,
        Duration::from_secs(30),
        /*telemetry*/ None,
    ));
    let error = rx
        .recv()
        .await
        .expect("terminal result")
        .expect_err("incomplete response");
    let error = crate::api_bridge::map_api_error(error).with_retry_after(
        codex_http_client::RetryAfter::from_delay(Duration::from_secs(1)).expect("retry deadline"),
    );
    assert_eq!(error.retry_delay(/*retry_count*/ 1), None);
    let codex_protocol::error::CodexErrorDetails::IncompleteResponse(failure) = error.details()
    else {
        panic!("expected terminal incomplete response, got {error:?}");
    };
    assert_eq!(failure.reason, "max_output_tokens");
    assert_eq!(failure.response_id.as_deref(), Some("resp-incomplete"));
    assert_eq!(
        failure.token_usage,
        Some(TokenUsage {
            input_tokens: 120,
            cached_input_tokens: 30,
            cache_write_input_tokens: 2,
            output_tokens: 50,
            reasoning_output_tokens: 40,
            total_tokens: 170,
            codex_rollout_budget_units: None,
        })
    );
    assert!(rx.recv().await.is_none());
    task.await.expect("SSE reader should exit");
}

#[test]
fn incomplete_response_stays_terminal_without_valid_usage() {
    for response in [
        json!({}),
        json!({"incomplete_details": {"reason": "content_filter"}, "usage": null}),
        json!({"incomplete_details": {"reason": "future_reason"}, "usage": {"input_tokens": "invalid"}}),
    ] {
        let event = serde_json::from_value(json!({
            "type": "response.incomplete", "response": response,
        }))
        .expect("parse event");
        let error = process_responses_event(event)
            .expect_err("terminal result")
            .into_api_error();
        let error = crate::api_bridge::map_api_error(error);
        assert_eq!(error.retry_delay(/*retry_count*/ 1), None);
        let codex_protocol::error::CodexErrorDetails::IncompleteResponse(failure) = error.details()
        else {
            panic!("expected terminal incomplete response, got {error:?}");
        };
        assert_eq!(failure.token_usage, None);
        assert_eq!(failure.response_id, None);
        assert_eq!(
            failure.reason,
            response["incomplete_details"]["reason"]
                .as_str()
                .unwrap_or("unknown")
        );
    }
}
