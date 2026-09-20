use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test_case::test_case("max_output_tokens"; "output_limit")]
#[test_case::test_case("content_filter"; "content_filter")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_response_does_not_resample_and_accounts_usage(reason: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    // Keep serving the same terminal response so every unintended retry is counted.
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            sse(vec![
                ev_response_created("resp-incomplete"),
                ev_assistant_message("msg-partial", "Retained partial answer"),
                incomplete_response_event(reason),
            ]),
            "text/event-stream",
        ))
        .mount(&server)
        .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        })
        .build_with_auto_env(&server)
        .await?;
    let events = submit_and_collect_events(&test).await?;
    let requests = server.received_requests().await.expect("recorded requests");
    let response_count = requests
        .iter()
        .filter(|request| request.method == "POST" && request.url.path() == "/v1/responses")
        .count();
    assert_eq!(response_count, 1, "a terminal response must not resample");
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventMsg::StreamError(_)))
            .count(),
        0,
        "terminal failures must not emit reconnect notifications"
    );
    let errors = events
        .iter()
        .filter_map(|event| match event {
            EventMsg::Error(error) => Some(error.message.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        errors,
        vec![format!("Incomplete response returned, reason: {reason}")]
    );
    let expected_usage = TokenUsage {
        input_tokens: 120,
        cached_input_tokens: 30,
        cache_write_input_tokens: 2,
        output_tokens: 50,
        reasoning_output_tokens: 40,
        total_tokens: 170,
        codex_rollout_budget_units: None,
    };
    let info = events
        .iter()
        .rev()
        .find_map(|event| match event {
            EventMsg::TokenCount(event) => event.info.as_ref(),
            _ => None,
        })
        .expect("the charged incomplete response must report usage");
    assert_eq!(info.last_token_usage, expected_usage);
    assert_eq!(info.total_token_usage, expected_usage);
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    test.codex.shutdown_and_wait().await?;
    let rollout = std::fs::read_to_string(rollout_path)?;
    let items = rollout
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let records = items
        .iter()
        .filter_map(|line| match &line.item {
            codex_history::RolloutItem::TokenUsageRecord(record) => Some(record),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].response_id, "resp-incomplete");
    assert_eq!(records[0].usage, expected_usage);
    assert_eq!(records[0].turn_token_usage, expected_usage);
    assert_eq!(records[0].thread_token_usage, expected_usage);
    let partial_output_count = items.iter().filter(|line| matches!(&line.item,
        codex_history::RolloutItem::ResponseItem(codex_history::ResponseItemEnvelope { item: codex_protocol::models::ResponseItem::Message { role, content, .. }, .. })
        if role == "assistant" && content.iter().any(|content| matches!(content,
            codex_protocol::models::ContentItem::OutputText { text }
            if text == "Retained partial answer"
        ))
    )).count();
    assert_eq!(partial_output_count, 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_response_does_not_retry_or_fall_back_from_websocket() -> Result<()> {
    use core_test_support::responses::ev_completed;
    use core_test_support::responses::start_websocket_server;

    skip_if_no_network!(Ok(()));
    let incomplete = vec![
        ev_response_created("resp-incomplete"),
        incomplete_response_event("max_output_tokens"),
    ];
    let server = start_websocket_server(vec![
        vec![
            vec![ev_response_created("prewarm"), ev_completed("prewarm")],
            incomplete.clone(),
        ],
        vec![incomplete.clone()],
        vec![incomplete],
    ])
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        })
        .build_with_websocket_server(&server)
        .await?;
    let events = submit_and_collect_events(&test).await?;
    let generation_requests = server
        .connections()
        .iter()
        .flatten()
        .filter(|request| request.body_json()["generate"] != false)
        .count();
    assert_eq!(generation_requests, 1);
    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EventMsg::StreamError(_)))
            .count(),
        0
    );
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, EventMsg::Warning(warning)
        if warning.message.contains("Falling back")))
    );
    let info = events
        .iter()
        .rev()
        .find_map(|event| match event {
            EventMsg::TokenCount(event) => event.info.as_ref(),
            _ => None,
        })
        .expect("incomplete WebSocket response usage");
    assert_eq!(info.total_token_usage.total_tokens, 170);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_response_usage_enforces_rollout_budget_without_retry() -> Result<()> {
    use codex_core::config::RolloutBudgetConfig;
    use codex_protocol::protocol::CodexErrorInfo;
    use core_test_support::responses::mount_sse_once;
    use core_test_support::wait_for_event;

    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut incomplete = incomplete_response_event("max_output_tokens");
    incomplete["response"]["usage"]["codex_rollout_budget_units"] = json!(120.5);
    let response = mount_sse_once(&server, sse(vec![incomplete])).await;
    let test = test_codex()
        .with_config(|config| {
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
            config.rollout_budget = Some(RolloutBudgetConfig {
                limit_tokens: 100,
                reminder_at_remaining_tokens: Vec::new(),
                sampling_token_weight: 1.0,
                prefill_token_weight: 1.0,
            });
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "produce an answer".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let usage = core_test_support::wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TokenCount(event) => event.info.clone(),
        _ => None,
    })
    .await;
    assert_eq!(usage.total_token_usage.total_tokens, 170);
    assert_eq!(
        usage.last_token_usage.codex_rollout_budget_units,
        Some(serde_json::Number::from_f64(120.5).expect("finite budget units"))
    );
    let error = wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await;
    let EventMsg::Error(error) = error else {
        unreachable!()
    };
    assert_eq!(
        error.codex_error_info,
        Some(CodexErrorInfo::SessionBudgetExceeded)
    );
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(response.requests().len(), 1);
    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.method == "POST" && request.url.path() == "/v1/responses")
            .count(),
        1
    );
    Ok(())
}

pub(super) fn incomplete_response_event(reason: &str) -> serde_json::Value {
    json!({
        "type": "response.incomplete",
        "response": {
            "id": "resp-incomplete",
            "status": "incomplete",
            "incomplete_details": {"reason": reason},
            "usage": {
                "input_tokens": 120,
                "input_tokens_details": {"cached_tokens": 30, "cache_write_tokens": 2},
                "output_tokens": 50,
                "output_tokens_details": {"reasoning_tokens": 40},
                "total_tokens": 170
            }
        }
    })
}

pub(super) async fn submit_and_collect_events(test: &TestCodex) -> Result<Vec<EventMsg>> {
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "produce an answer".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut events = Vec::new();
        loop {
            let event = test.codex.next_event().await?.msg;
            let completed = matches!(event, EventMsg::TurnComplete(_));
            events.push(event);
            if completed {
                return Ok(events);
            }
        }
    })
    .await?
}
