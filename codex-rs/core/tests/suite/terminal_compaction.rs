//! Terminal failure and usage handling for local and remote compaction.

use super::terminal_response::incomplete_response_event;
use super::terminal_response::submit_and_collect_events;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test_case::test_case("credit_balance_exhausted"; "quota")]
#[test_case::test_case("invalid_prompt"; "invalid_request")]
#[test_case::test_case("max_output_tokens"; "incomplete")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_compaction_does_not_retry_terminal_failure(code: &str) -> Result<()> {
    use codex_protocol::protocol::Op;
    use core_test_support::responses::ev_completed;
    use core_test_support::responses::mount_sse_once;
    use core_test_support::wait_for_event;

    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("seed"),
            ev_assistant_message("seed-message", "seed answer"),
            ev_completed("seed"),
        ]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_provider.name = "Local audit provider".to_string();
            config.compact_prompt = Some("audit compaction request".to_string());
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "seed turn".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let terminal_event = if code == "max_output_tokens" {
        incomplete_response_event(code)
    } else {
        json!({
            "type": "response.failed",
            "response": {
                "id": "failed-compaction",
                "error": {"code": code, "message": "terminal compaction failure"}
            }
        })
    };
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse(vec![terminal_event]), "text/event-stream"),
        )
        .mount(&server)
        .await;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = server.received_requests().await.expect("recorded requests");
    let compact_requests = requests
        .iter()
        .filter(|request| {
            request.method == "POST"
                && request.url.path() == "/v1/responses"
                && String::from_utf8_lossy(&request.body).contains("audit compaction request")
        })
        .count();
    assert_eq!(
        compact_requests, 1,
        "terminal compaction failure must not retry"
    );
    Ok(())
}

#[test_case::test_case("Local audit provider"; "local")]
#[test_case::test_case("OpenAI"; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_compaction_records_usage_once(provider_name: &str) -> Result<()> {
    use codex_history::RolloutItem;
    use codex_protocol::protocol::Op;
    use core_test_support::responses::ev_completed;
    use core_test_support::responses::mount_sse_once;
    use core_test_support::wait_for_event;

    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    mount_sse_once(&server, sse(vec![ev_completed("seed")])).await;
    let name = provider_name.to_string();
    let test = test_codex()
        .with_config(move |config| {
            config.model_provider.name = name;
            config.compact_prompt = Some("audit compaction request".to_string());
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        })
        .build_with_auto_env(&server)
        .await?;
    submit_and_collect_events(&test).await?;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse(vec![incomplete_response_event("max_output_tokens")]),
            "text/event-stream",
        ))
        .mount(&server)
        .await;
    test.codex.submit(Op::Compact).await?;
    let mut events = Vec::new();
    wait_for_event(&test.codex, |event| {
        events.push(event.clone());
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventMsg::Error(_)))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventMsg::StreamError(_)))
            .count(),
        0
    );
    let usage = events
        .iter()
        .rev()
        .find_map(|event| match event {
            EventMsg::TokenCount(event) => event.info.as_ref(),
            _ => None,
        })
        .expect("compaction usage event");
    assert_eq!(usage.last_token_usage.total_tokens, 170);
    assert_eq!(usage.total_token_usage.total_tokens, 170);
    let requests = server.received_requests().await.expect("recorded requests");
    let responses = requests
        .iter()
        .filter(|request| request.method == "POST" && request.url.path() == "/v1/responses")
        .collect::<Vec<_>>();
    assert_eq!(
        responses.len(),
        2,
        "one seed request and one compaction request"
    );
    let compact_body = String::from_utf8_lossy(&responses[1].body);
    match provider_name {
        "OpenAI" => assert!(compact_body.contains("compaction_trigger")),
        "Local audit provider" => assert!(compact_body.contains("audit compaction request")),
        _ => unreachable!("test provider"),
    }
    test.codex.flush_rollout().await?;
    let history = test.codex.load_history(/*include_archived*/ false).await?;
    let records = history
        .items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::TokenUsageRecord(record) if record.response_id == "resp-incomplete" => {
                Some(record)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].usage, usage.last_token_usage);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_compaction_still_retries_transient_failures() -> Result<()> {
    use codex_protocol::protocol::Op;
    use core_test_support::responses::ev_completed;
    use core_test_support::responses::mount_sse_once;
    use core_test_support::wait_for_event;

    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    mount_sse_once(&server, sse(vec![ev_completed("seed")])).await;
    let test = test_codex()
        .with_config(|config| {
            config.model_provider.name = "Local audit provider".to_string();
            config.compact_prompt = Some("audit compaction request".to_string());
            config.model_provider.stream_max_retries = Some(1);
            config.model_provider.request_max_retries = Some(0);
        })
        .build_with_auto_env(&server)
        .await?;
    submit_and_collect_events(&test).await?;
    mount_sse_once(
        &server,
        sse(vec![json!({
            "type": "response.failed", "response": {
                "id": "transient", "error": {"code": "server_error", "message": "temporary failure"}
            }
        })]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("summary", "Compacted seed conversation"),
            ev_completed("compacted"),
        ]),
    )
    .await;
    test.codex.submit(Op::Compact).await?;
    let mut events = Vec::new();
    wait_for_event(&test.codex, |event| {
        events.push(event.clone());
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventMsg::StreamError(_)))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventMsg::Error(_)))
            .count(),
        0
    );
    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.method == "POST" && request.url.path() == "/v1/responses")
            .count(),
        3,
        "seed, transient failure, successful retry"
    );
    Ok(())
}
