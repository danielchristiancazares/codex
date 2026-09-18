use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn incomplete_response_preserves_accounting_and_is_terminal() {
    let raw = json!({
        "type": "response.incomplete",
        "response": {
            "id": "resp-incomplete",
            "incomplete_details": { "reason": "max_output_tokens" },
            "usage": {
                "input_tokens": 10,
                "input_tokens_details": { "cached_tokens": 3, "cache_write_tokens": 2 },
                "output_tokens": 7,
                "output_tokens_details": { "reasoning_tokens": 4 },
                "total_tokens": 17
            }
        }
    })
    .to_string();
    let event = serde_json::from_str(&raw).unwrap();
    let Err(ResponsesEventError::Api(ApiError::IncompleteResponse(failure))) =
        process_responses_event(event, &raw)
    else {
        panic!("expected terminal incomplete response");
    };
    let mut recorded = Vec::new();
    failure
        .record_usage(|usage| {
            recorded.push(usage.clone());
            std::future::ready(())
        })
        .await;
    assert_eq!(
        recorded,
        vec![TokenUsage {
            input_tokens: 10,
            cached_input_tokens: 3,
            cache_write_input_tokens: 2,
            output_tokens: 7,
            reasoning_output_tokens: 4,
            total_tokens: 17,
            ..Default::default()
        }]
    );
    let error = crate::api_bridge::map_api_error(ApiError::IncompleteResponse(failure));
    assert!(!error.is_retryable());
    assert!(error.to_string().contains("resp-incomplete"));
}

#[test]
fn required_event_fields_fail_with_bounded_diagnostics() {
    let cases = [
        json!({"type": "response.output_item.done"}),
        json!({"type": "response.output_item.added", "item": {"type": "message"}}),
        json!({"type": "response.output_text.delta"}),
        json!({"type": "response.custom_tool_call_input.delta", "delta": "{"}),
        json!({"type": "response.reasoning_summary_text.delta", "delta": "x"}),
        json!({"type": "response.reasoning_summary_text.done", "text": "x"}),
        json!({"type": "response.reasoning_text.delta", "delta": "x"}),
        json!({"type": "response.created"}),
        json!({"type": "response.failed", "response": {}}),
        json!({"type": "response.incomplete"}),
        json!({"type": "response.completed", "response": {}}),
        json!({"type": "response.reasoning_summary_part.added"}),
    ];
    let prefix = "x".repeat(2048 - "…".len() - 1);
    let raw = format!("{prefix}🦀suffix");
    for value in cases {
        let event = serde_json::from_value(value).unwrap();
        let Err(ResponsesEventError::Api(ApiError::ResponseProtocol(failure))) =
            process_responses_event(event, &raw)
        else {
            panic!("expected protocol failure");
        };
        assert_eq!(failure.diagnostic(), format!("{prefix}…"));
        assert!(
            !crate::api_bridge::map_api_error(ApiError::ResponseProtocol(failure)).is_retryable()
        );
    }
}

#[test]
fn null_failed_details_still_retry_and_extensions_remain_compatible() {
    let event = serde_json::from_value(json!({
        "type": "response.failed", "response": {"error": null}
    }))
    .unwrap();
    let error = process_responses_event(event, "{}")
        .unwrap_err()
        .into_api_error();
    assert!(crate::api_bridge::map_api_error(error).is_retryable());
    let event = serde_json::from_value(json!({"type": "response.future_extension"})).unwrap();
    assert!(process_responses_event(event, "{}").unwrap().is_none());
}

#[tokio::test]
async fn malformed_sse_stops_without_waiting_for_stream_closure() {
    let (tx, mut rx) = mpsc::channel(4);
    let stream =
        futures::stream::once(async { Ok(bytes::Bytes::from_static(b"data: {invalid json\n\n")) })
            .chain(futures::stream::pending());
    let task = tokio::spawn(process_sse(
        Box::pin(stream),
        tx,
        Duration::from_secs(30),
        None,
    ));
    let result = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(ApiError::ResponseProtocol(_))));
    task.await.unwrap();
}
