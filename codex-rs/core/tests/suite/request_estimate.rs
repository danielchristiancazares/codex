//! Request estimates must not prevent provider acceptance or bypass compaction safeguards.

use anyhow::Result;
use codex_core::CodexThread;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use tracing_test::traced_test;
use wiremock::MockServer;

const CONTEXT_WINDOW: i64 = 32_000;
const SUMMARY: &str = "accepted compaction summary";

#[derive(Clone, Copy)]
enum Compaction {
    Legacy,
    V2,
}

#[derive(Clone, Copy)]
enum Format {
    Responses,
    Lite,
}

#[derive(Clone, Copy)]
enum Trigger {
    Manual,
    Automatic,
}

fn builder(compaction: Compaction, format: Format) -> TestCodexBuilder {
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.4", move |model| {
            model.use_responses_lite = matches!(format, Format::Lite);
        })
        .with_config(move |config| {
            config.base_instructions = Some("Follow the user's instructions.".to_string());
            config.model_context_window = Some(CONTEXT_WINDOW);
            config.model_auto_compact_token_limit = Some(28_000);
            match compaction {
                Compaction::Legacy => config.features.disable(Feature::RemoteCompactionV2),
                Compaction::V2 => config.features.enable(Feature::RemoteCompactionV2),
            }
            .expect("configure remote compaction");
        })
}

async fn add_large_tool(test: &mut TestCodex) -> Result<()> {
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: "large_tool".to_string(),
                description: "tool detail ".repeat(/*n*/ 4_000),
                input_schema: json!({"type": "object", "properties": {}}),
                defer_loading: false,
            })],
            environments: Some(vec![test.executor_environment().selection().clone()]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    test.codex = thread.thread;
    test.session_configured = thread.session_configured;
    Ok(())
}

async fn submit(codex: &CodexThread, text: &str) -> Result<()> {
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    Ok(())
}

async fn finish(codex: &CodexThread) {
    wait_for_event(codex, |event| match event {
        EventMsg::Error(error) => panic!("unexpected turn error: {error:?}"),
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;
}

fn assistant_response(id: &str, text: &str, tokens: i64) -> Vec<Value> {
    vec![
        responses::ev_response_created(id),
        responses::ev_assistant_message(&format!("{id}-message"), text),
        responses::ev_completed_with_tokens(id, tokens),
    ]
}

fn compaction_response(summary: &str) -> Vec<Value> {
    vec![
        json!({
            "type": "response.output_item.done",
            "item": {"type": "compaction", "encrypted_content": summary},
        }),
        responses::ev_completed("compaction"),
    ]
}

async fn assert_request_count(server: &MockServer, expected: usize) {
    let requests = server.received_requests().await.expect("received requests");
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path().starts_with("/v1/responses"))
            .count(),
        expected,
    );
}

#[traced_test]
#[test_case(Format::Responses; "responses")]
#[test_case(Format::Lite; "lite")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overestimated_inference_is_sent_once(format: Format) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse(assistant_response("accepted", "done", /*tokens*/ 100)),
    )
    .await;
    let mut test = builder(Compaction::Legacy, format)
        .with_config(|config| config.model_context_window = Some(10_000))
        .build_with_auto_env(&server)
        .await?;
    add_large_tool(&mut test).await?;

    submit(&test.codex, "Use these tools if needed.").await?;
    let usage = wait_for_event(&test.codex, |event| match event {
        EventMsg::Error(error) => panic!("unexpected turn error: {error:?}"),
        EventMsg::TokenCount(event) => event
            .info
            .as_ref()
            .is_some_and(|info| info.last_token_usage.total_tokens > 0),
        _ => false,
    })
    .await;
    let EventMsg::TokenCount(usage) = usage else {
        unreachable!();
    };
    assert_eq!(
        usage
            .info
            .expect("provider usage")
            .last_token_usage
            .total_tokens,
        100
    );
    finish(&test.codex).await;

    let body = mock.single_request().body_json();
    assert!(
        body.to_string()
            .contains(&"tool detail ".repeat(/*n*/ 4_000))
    );
    assert_request_count(&server, /*expected*/ 1).await;
    // Session tasks have their own spans; inspect the estimator's tracing target.
    tracing_test::internal::logs_assert("codex_core::client::request_estimate", |lines| {
        let diagnostic = lines
            .iter()
            .find(|line| line.contains("Model request estimate exceeds usable context window"))
            .ok_or_else(|| "missing request estimate diagnostic".to_string())?;
        for field in [
            "input_tokens=",
            "instructions_tokens=",
            "schema_tokens=",
            "estimated_tokens=",
            "usable_context_window=9500",
            "model=gpt-5.4",
            "thread_id=",
            "turn_id=",
        ] {
            assert!(
                diagnostic.contains(field),
                "missing diagnostic field: {field}"
            );
        }
        assert!(!diagnostic.contains("tool detail"));
        assert!(!diagnostic.contains("Use these tools"));
        Ok(())
    })
    .expect("request estimate diagnostic");
    Ok(())
}

#[test_case(Compaction::Legacy, Format::Responses, Trigger::Manual; "legacy_manual")]
#[test_case(Compaction::Legacy, Format::Lite, Trigger::Manual; "legacy_lite_manual")]
#[test_case(Compaction::V2, Format::Responses, Trigger::Manual; "v2_manual")]
#[test_case(Compaction::V2, Format::Lite, Trigger::Manual; "v2_lite_manual")]
#[test_case(Compaction::Legacy, Format::Responses, Trigger::Automatic; "legacy_auto")]
#[test_case(Compaction::Legacy, Format::Lite, Trigger::Automatic; "legacy_lite_auto")]
#[test_case(Compaction::V2, Format::Responses, Trigger::Automatic; "v2_auto")]
#[test_case(Compaction::V2, Format::Lite, Trigger::Automatic; "v2_lite_auto")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overestimated_compaction_reaches_provider(
    compaction: Compaction,
    format: Format,
    trigger: Trigger,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let history = "history ".repeat(/*n*/ 10_000);
    let tokens = match trigger {
        Trigger::Manual => 100,
        Trigger::Automatic => 31_000,
    };
    let mut replies = vec![responses::sse(assistant_response("seed", &history, tokens))];
    let compact = match compaction {
        Compaction::Legacy => {
            Some(responses::mount_compact_user_history_with_summary_once(&server, SUMMARY).await)
        }
        Compaction::V2 => {
            replies.push(responses::sse(compaction_response(SUMMARY)));
            None
        }
    };
    replies.push(responses::sse(assistant_response(
        "continued",
        "done",
        /*tokens*/ 100,
    )));
    let mock = responses::mount_sse_sequence(&server, replies).await;
    let mut test = builder(compaction, format)
        .build_with_auto_env(&server)
        .await?;
    add_large_tool(&mut test).await?;

    submit(&test.codex, "Seed the history.").await?;
    finish(&test.codex).await;
    if matches!(trigger, Trigger::Manual) {
        test.codex.submit(Op::Compact).await?;
        finish(&test.codex).await;
    }
    submit(&test.codex, "Continue after compaction.").await?;
    finish(&test.codex).await;

    let requests = mock.requests();
    let compact_body = match compact {
        Some(compact) => compact.single_request().body_json(),
        None => requests[1].body_json(),
    };
    // History alone fits the window; adding the tool description takes it over.
    // The provider receives both intact, then exactly one inference uses its summary.
    assert!(compact_body.to_string().contains(&history));
    assert!(
        compact_body
            .to_string()
            .contains(&"tool detail ".repeat(/*n*/ 4_000))
    );
    let continuation = requests.last().expect("continuation").body_json();
    assert!(continuation.to_string().contains(SUMMARY));
    assert!(!continuation.to_string().contains(&history));
    assert_request_count(&server, /*expected*/ 3).await;
    Ok(())
}

#[test_case(Compaction::Legacy; "legacy")]
#[test_case(Compaction::V2; "v2")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unusable_compaction_replacement_stops_turn(compaction: Compaction) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let oversized_summary = "x".repeat(/*n*/ 200_000);
    let mut replies = vec![responses::sse(assistant_response(
        "seed",
        "preserve this history",
        /*tokens*/ 31_000,
    ))];
    let compact = match compaction {
        Compaction::Legacy => Some(
            responses::mount_compact_user_history_with_summary_once(&server, &oversized_summary)
                .await,
        ),
        Compaction::V2 => {
            replies.push(responses::sse(compaction_response(&oversized_summary)));
            None
        }
    };
    let mock = responses::mount_sse_sequence(&server, replies).await;
    let test = builder(compaction, Format::Responses)
        .build_with_auto_env(&server)
        .await?;
    submit(&test.codex, "Seed the history.").await?;
    finish(&test.codex).await;
    let before = test.codex.conversation_history_snapshot().await;

    submit(&test.codex, "Trigger automatic compaction.").await?;
    let error = wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await;
    let EventMsg::Error(error) = error else {
        unreachable!();
    };
    assert!(error.message.contains("compaction replacement cannot fit"));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let after = test.codex.conversation_history_snapshot().await;
    assert_eq!(
        after.items().collect::<Vec<_>>(),
        before.items().collect::<Vec<_>>()
    );
    match compact {
        Some(compact) => {
            compact.single_request();
        }
        None => assert_eq!(mock.requests().len(), 2),
    }
    assert_request_count(&server, /*expected*/ 2).await;
    Ok(())
}

#[test_case(Format::Responses; "responses")]
#[test_case(Format::Lite; "lite")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_context_rejection_is_not_retried(format: Format) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse_failed(
            "rejected",
            "context_length_exceeded",
            "provider rejected request",
        ),
    )
    .await;
    let mut test = builder(Compaction::Legacy, format)
        .with_config(|config| config.model_context_window = Some(10_000))
        .build_with_auto_env(&server)
        .await?;
    add_large_tool(&mut test).await?;

    submit(&test.codex, "Try this request.").await?;
    let error = wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await;
    let EventMsg::Error(error) = error else {
        unreachable!();
    };
    assert!(error.message.contains("context window"));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    mock.single_request();
    assert_request_count(&server, /*expected*/ 1).await;
    Ok(())
}

#[test_case(Format::Responses; "responses")]
#[test_case(Format::Lite; "lite")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overestimated_v2_compaction_over_websocket(format: Format) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let history = "history ".repeat(/*n*/ 10_000);
    let server = responses::start_websocket_server(vec![vec![
        vec![
            responses::ev_response_created("warm"),
            responses::ev_completed("warm"),
        ],
        assistant_response("seed", &history, /*tokens*/ 31_000),
        compaction_response(SUMMARY),
        assistant_response("continued", "done", /*tokens*/ 100),
    ]])
    .await;
    let bootstrap_server = MockServer::start().await;
    let mut test = builder(Compaction::V2, format)
        .build_with_auto_env(&bootstrap_server)
        .await?;
    // Only the thread with dynamic tools should open a WebSocket connection.
    test.config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
    test.config.model_provider.supports_websockets = true;
    add_large_tool(&mut test).await?;
    submit(&test.codex, "Seed the history.").await?;
    finish(&test.codex).await;
    submit(&test.codex, "Continue after compaction.").await?;
    finish(&test.codex).await;

    let requests = server.single_connection();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].body_json()["generate"], json!(false));
    let compact = requests[2].body_json();
    // The server already has the large assistant response and tool definitions.
    // Compaction must preserve incremental transport reuse and send only the trigger.
    assert_eq!(compact["previous_response_id"], json!("seed"));
    assert_eq!(compact["input"], json!([{"type": "compaction_trigger"}]));
    assert!(requests[..2].iter().any(|request| {
        request
            .body_json()
            .to_string()
            .contains(&"tool detail ".repeat(/*n*/ 4_000))
    }));
    assert!(requests[3].body_json().to_string().contains(SUMMARY));
    server.shutdown().await;
    Ok(())
}
