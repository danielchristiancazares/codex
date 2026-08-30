use anyhow::Result;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;

use super::compact::non_openai_model_provider;
use super::compact::set_test_compact_prompt;

const FAILED_OUTPUT: &str = "FAILED_LOCAL_COMPACTION_OUTPUT_MUST_NOT_PERSIST";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_local_compaction_output_is_absent_after_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_log = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("initial-message", "before failure"),
                responses::ev_completed("initial-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("failed-message", FAILED_OUTPUT),
                json!({
                    "type": "response.failed",
                    "response": {
                        "id": "failed-compaction",
                        "error": {
                            "code": "server_error",
                            "message": "failed after output"
                        }
                    }
                }),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("resumed-message", "after resume"),
                responses::ev_completed("resumed-response"),
            ]),
        ],
    )
    .await;
    let mut provider = non_openai_model_provider(&server);
    provider.stream_max_retries = Some(0);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = provider;
        set_test_compact_prompt(config);
    });
    let initial = builder.build(&server).await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    initial
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "before failure".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    initial.codex.submit(Op::Compact).await?;
    wait_for_event(&initial.codex, |event| matches!(event, EventMsg::Error(_))).await;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    initial.codex.flush_rollout().await?;
    let persisted = fs::read_to_string(&rollout_path)?;
    assert!(!persisted.contains(FAILED_OUTPUT));

    initial.codex.submit(Op::Shutdown).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    let provider = non_openai_model_provider(&server);
    let mut resumed_builder = test_codex().with_config(move |config| {
        config.model_provider = provider;
        set_test_compact_prompt(config);
    });
    let resumed = resumed_builder.resume(&server, home, rollout_path).await?;
    resumed
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "after resume".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_log.requests();
    assert_eq!(requests.len(), 3);
    let resumed_request = requests[2].body_json().to_string();
    assert!(!resumed_request.contains(FAILED_OUTPUT));
    assert!(resumed_request.contains("before failure"));
    assert!(resumed_request.contains("after resume"));
    Ok(())
}
