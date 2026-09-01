use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_models_manager::bundled_models_response;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once_match;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;

const PARENT_MODEL: &str = "gpt-5.2";
const CHILD_MODEL: &str = "gpt-5.4";
const PARENT_CONTEXT_WINDOW: i64 = 300_000;
const PARENT_AUTO_COMPACT_LIMIT: i64 = 200_000;
const CHILD_CONTEXT_WINDOW: i64 = 200_000;
const CHILD_AUTO_COMPACT_LIMIT: i64 = 100_000;
const STALE_PARENT_TOKENS: i64 = 150_000;
const SEED_PROMPT: &str = "seed retained fork history";
const SPAWN_PROMPT: &str = "spawn a forked child";
const CHILD_PROMPT: &str = "inspect the retained context";
const SPAWN_CALL_ID: &str = "spawn-stale-token-child";
const MULTI_AGENT_V1_NAMESPACE: &str = "multi_agent_v1";

fn request_body_contains(request: &wiremock::Request, text: &str) -> bool {
    let is_zstd = request
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    let body = if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body)).ok()
    } else {
        Some(request.body.clone())
    };
    body.and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn filtered_legacy_fork_recomputes_usage_before_its_first_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _seed_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            request_body_contains(request, SEED_PROMPT)
                && !request_body_contains(request, SPAWN_PROMPT)
        },
        sse(vec![
            ev_response_created("seed-response"),
            ev_assistant_message("seed-message", "seed complete"),
            ev_completed_with_tokens("seed-response", STALE_PARENT_TOKENS),
        ]),
    )
    .await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "fork_context": true,
        "model": CHILD_MODEL,
    }))?;
    let _spawn_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            request_body_contains(request, SPAWN_PROMPT)
                && !request_body_contains(request, CHILD_PROMPT)
        },
        sse(vec![
            ev_response_created("spawn-response"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V1_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("spawn-response"),
        ]),
    )
    .await;
    let child_request = mount_response_once_match(
        &server,
        |request: &wiremock::Request| {
            request_body_contains(request, CHILD_PROMPT)
                && !request_body_contains(request, SPAWN_CALL_ID)
        },
        sse_response(sse(vec![
            ev_response_created("child-response"),
            ev_assistant_message("child-message", "child complete"),
            ev_completed("child-response"),
        ]))
        .set_delay(Duration::from_millis(/*millis*/ 200)),
    )
    .await;
    let _parent_followup = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("parent-followup-response"),
            ev_assistant_message("parent-followup-message", "parent complete"),
            ev_completed("parent-followup-response"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        let model_catalog = config.model_catalog.get_or_insert_with(|| {
            bundled_models_response().expect("bundled models.json should parse")
        });
        for (model, context_window, auto_compact_token_limit) in [
            (
                PARENT_MODEL,
                PARENT_CONTEXT_WINDOW,
                PARENT_AUTO_COMPACT_LIMIT,
            ),
            (CHILD_MODEL, CHILD_CONTEXT_WINDOW, CHILD_AUTO_COMPACT_LIMIT),
        ] {
            let model_info = model_catalog
                .models
                .iter_mut()
                .find(|model_info| model_info.slug == model)
                .unwrap_or_else(|| panic!("{model} should exist in bundled models.json"));
            model_info.context_window = Some(context_window);
            model_info.auto_compact_token_limit = Some(auto_compact_token_limit);
            model_info.effective_context_window_percent = 100;
        }
        config.model = Some(PARENT_MODEL.to_string());
        config.model_context_window = None;
        config.model_auto_compact_token_limit = None;
    });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_text_turn(SEED_PROMPT).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: SPAWN_PROMPT.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let child_thread_id = {
        let deadline = Instant::now() + Duration::from_secs(/*secs*/ 5);
        loop {
            let child_thread_id = test
                .thread_manager
                .list_thread_ids()
                .await
                .into_iter()
                .find(|thread_id| *thread_id != test.session_configured.thread_id);
            if let Some(child_thread_id) = child_thread_id {
                break child_thread_id;
            }
            if Instant::now() >= deadline {
                anyhow::bail!("timed out waiting for forked child thread");
            }
            sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    };
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    let initial_usage = child_thread
        .token_usage_info()
        .await
        .expect("forked child should seed estimated token usage");
    let estimated_child_tokens = initial_usage.last_token_usage.total_tokens;
    assert!(
        (1..CHILD_AUTO_COMPACT_LIMIT).contains(&estimated_child_tokens),
        "retained child history should fit its first-turn compaction limit: {initial_usage:?}"
    );
    assert_eq!(
        initial_usage,
        TokenUsageInfo {
            total_token_usage: TokenUsage::default(),
            last_token_usage: TokenUsage {
                total_tokens: estimated_child_tokens,
                ..TokenUsage::default()
            },
            model_context_window: Some(CHILD_CONTEXT_WINDOW),
        }
    );

    wait_for_event(&child_thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let child_thread_id = child_thread_id.to_string();
    let child_requests = child_request
        .requests()
        .into_iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["thread_id"].as_str()
                == Some(child_thread_id.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(child_requests.len(), 1);
    let request = child_requests
        .into_iter()
        .next()
        .expect("child should make its first model request");
    assert_eq!(request.body_json()["model"], CHILD_MODEL);
    assert!(request.body_contains_text(SEED_PROMPT));
    assert!(request.body_contains_text(CHILD_PROMPT));

    Ok(())
}
