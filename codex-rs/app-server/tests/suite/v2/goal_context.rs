use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadGoalGetResponse;
use codex_app_server_protocol::ThreadGoalSetResponse;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::skip_if_no_network;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
#[cfg(not(windows))]
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CONTINUATION_COUNT: usize = 10;
const OBJECTIVE: &str = r"finish <file & notes> from C:\tmp\goal.md";
const RENDERED_OBJECTIVE: &str = r"finish &lt;file &amp; notes&gt; from C:\tmp\goal.md";
const DELTA_START: &str =
    "The persisted thread goal is active. Continue it from the current authoritative state";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn goal_continuations_reuse_one_context_revision_and_append_bounded_deltas() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut websocket_requests = vec![
        vec![
            responses::ev_response_created("warmup"),
            responses::ev_completed("warmup"),
        ],
        vec![
            responses::ev_response_created("materialize-thread"),
            responses::ev_completed("materialize-thread"),
        ],
    ];
    websocket_requests.extend((1..=CONTINUATION_COUNT).map(|index| {
        let response_id = format!("goal-continuation-{index}");
        vec![
            responses::ev_response_created(&response_id),
            responses::ev_completed_with_tokens(&response_id, /*total_tokens*/ 1),
        ]
    }));
    let websocket_server = responses::start_websocket_server(vec![websocket_requests]).await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&websocket_server.uri().replacen("ws://", "http://", 1))
        .with_model("gpt-5.4")
        .enable_feature(Feature::Goals)
        .with_provider_config("supports_websockets = true")
        .write(codex_home.path())?;

    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await?;
    let start_id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(READ_TIMEOUT, app.read_response(start_id)).await??;

    let initial_turn_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(READ_TIMEOUT, app.read_response(initial_turn_id)).await??;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let set_id = app
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id.clone(),
                "objective": OBJECTIVE,
                "tokenBudget": CONTINUATION_COUNT,
            })),
        )
        .await?;
    let set: ThreadGoalSetResponse = timeout(READ_TIMEOUT, app.read_response(set_id)).await??;
    assert_eq!(set.goal.status, ThreadGoalStatus::Active);

    for _ in 0..CONTINUATION_COUNT {
        timeout(
            READ_TIMEOUT,
            app.read_stream_until_notification_message("turn/completed"),
        )
        .await??;
    }

    let get_id = app
        .send_raw_request(
            "thread/goal/get",
            Some(json!({
                "threadId": thread.id.clone(),
            })),
        )
        .await?;
    let get: ThreadGoalGetResponse = timeout(READ_TIMEOUT, app.read_response(get_id)).await??;
    let goal = get.goal.expect("goal should remain persisted");
    assert_eq!(goal.status, ThreadGoalStatus::BudgetLimited);
    assert_eq!(goal.tokens_used, CONTINUATION_COUNT as i64);

    let mut continuation_payloads = Vec::with_capacity(CONTINUATION_COUNT);
    for index in 0..CONTINUATION_COUNT {
        continuation_payloads.push(
            websocket_server
                .wait_for_request(
                    /*connection_index*/ 0,
                    /*request_index*/ index + 2,
                )
                .await
                .body_json(),
        );
    }
    for (index, payload) in continuation_payloads.iter().enumerate() {
        let mut strings = Vec::new();
        collect_strings(&payload["input"], &mut strings);
        assert_eq!(
            strings
                .iter()
                .filter(|text| text.contains("<goal_context_revision>"))
                .count(),
            usize::from(index == 0),
            "only the first continuation request should send the static revision; request {}",
            index + 1
        );
        assert_eq!(
            strings
                .iter()
                .filter(|text| text.contains(RENDERED_OBJECTIVE))
                .count(),
            usize::from(index == 0),
            "only the first continuation request should send the objective; request {}",
            index + 1
        );
        assert_eq!(
            strings
                .iter()
                .filter(|text| text.contains(DELTA_START))
                .count(),
            1,
            "continuation request {} should send one delta",
            index + 1
        );
    }

    let mut continuation_strings = Vec::new();
    for payload in &continuation_payloads {
        collect_strings(&payload["input"], &mut continuation_strings);
    }
    assert_eq!(
        continuation_strings
            .iter()
            .filter(|text| text.contains("<goal_context_revision>"))
            .count(),
        1
    );
    assert_eq!(
        continuation_strings
            .iter()
            .filter(|text| text.contains(RENDERED_OBJECTIVE))
            .count(),
        1
    );
    let deltas = continuation_strings
        .into_iter()
        .filter(|text| text.contains(DELTA_START))
        .collect::<Vec<_>>();
    assert_eq!(deltas.len(), CONTINUATION_COUNT);
    assert!(deltas.iter().all(|delta| delta.len() <= 512));
    assert!(deltas.iter().all(|delta| !delta.contains(OBJECTIVE)));
    assert!(
        deltas
            .iter()
            .all(|delta| !delta.contains(RENDERED_OBJECTIVE))
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cf_030_interrupted_goal_turn_does_not_continue() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let websocket_server =
        responses::start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
            requests: vec![
                vec![
                    responses::ev_response_created("warmup"),
                    responses::ev_completed("warmup"),
                ],
                vec![
                    responses::ev_response_created("materialize-thread"),
                    responses::ev_completed("materialize-thread"),
                ],
                vec![responses::ev_response_created("interrupt-goal")],
            ],
            response_headers: Vec::new(),
            accept_delay: None,
            close_after_requests: false,
        }])
        .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&websocket_server.uri().replacen("ws://", "http://", 1))
        .with_model("gpt-5.4")
        .enable_feature(Feature::Goals)
        .with_provider_config("supports_websockets = true")
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized_with_timeout(READ_TIMEOUT)
        .await?;
    let start_id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("gpt-5.4".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(READ_TIMEOUT, app.read_response(start_id)).await??;
    let materialize_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "materialize this thread".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(READ_TIMEOUT, app.read_response(materialize_id)).await??;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/started"),
    )
    .await??;
    timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let set_id = app
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.id.clone(),
                "objective": "stay interrupted",
            })),
        )
        .await?;
    let _: ThreadGoalSetResponse = timeout(READ_TIMEOUT, app.read_response(set_id)).await??;
    let started = timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/started"),
    )
    .await??;
    websocket_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 2)
        .await;
    let turn_id = started
        .params
        .as_ref()
        .and_then(|params| params.get("turn"))
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .expect("turn/started notification should contain the turn id")
        .to_string();

    app.interrupt_turn_and_wait_for_aborted(thread.id.clone(), turn_id, READ_TIMEOUT)
        .await?;
    tokio::time::sleep(Duration::from_millis(250)).await;
    let response_create_count = websocket_server
        .single_connection()
        .into_iter()
        .filter(|request| request.body_json()["type"] == "response.create")
        .count();
    assert_eq!(response_create_count, 3);

    let get_id = app
        .send_raw_request("thread/goal/get", Some(json!({ "threadId": thread.id })))
        .await?;
    let get: ThreadGoalGetResponse = timeout(READ_TIMEOUT, app.read_response(get_id)).await??;
    assert_eq!(
        get.goal.expect("goal should remain persisted").status,
        ThreadGoalStatus::Active
    );
    websocket_server.shutdown().await;
    Ok(())
}

fn collect_strings<'a>(value: &'a Value, strings: &mut Vec<&'a str>) {
    match value {
        Value::String(value) => strings.push(value),
        Value::Array(values) => {
            for value in values {
                collect_strings(value, strings);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_strings(value, strings);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
