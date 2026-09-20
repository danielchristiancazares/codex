use super::collect_compaction_output;
use crate::ResponseStream;
use crate::agent::control::LocalAgentControl;
use crate::client_common::ResponseEvent;
use crate::config::RolloutBudgetConfig;
use crate::session::session::Session;
use crate::session::tests::make_session_and_context;
use crate::session::turn_context::TurnContext;
use crate::thread_manager::default_thread_id_generator;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsage;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

async fn budget_session(limit_tokens: i64) -> (Session, TurnContext) {
    let (mut sess, turn_context) = make_session_and_context().await;
    sess.services.agent_control = LocalAgentControl::new(
        std::sync::Weak::default(),
        default_thread_id_generator(),
        Some(RolloutBudgetConfig {
            limit_tokens,
            reminder_at_remaining_tokens: vec![],
            sampling_token_weight: 1.0,
            prefill_token_weight: 1.0,
        }),
    );
    (sess, turn_context)
}

fn remaining_budget(sess: &Session) -> i64 {
    sess.services
        .agent_control
        .pending_budget_reminder(sess.thread_id(), "compaction-budget-test")
        .expect("configured budget")
        .remaining_tokens
}

fn usage() -> TokenUsage {
    TokenUsage {
        input_tokens: 20,
        cached_input_tokens: 10,
        output_tokens: 5,
        total_tokens: 25,
        ..Default::default()
    }
}

fn completed_stream(count: usize, token_usage: Option<TokenUsage>) -> ResponseStream {
    let mut events = (0..count)
        .map(|_| {
            Ok(ResponseEvent::OutputItemDone(ResponseItem::Compaction {
                id: None,
                encrypted_content: "encrypted".to_string(),
                internal_chat_message_metadata_passthrough: None,
            }))
        })
        .collect::<Vec<_>>();
    events.push(Ok(ResponseEvent::Completed {
        response_id: "resp-budget".to_string(),
        token_usage,
        usage_metadata: None,
        end_turn: Some(true),
    }));
    response_stream(events)
}

fn response_stream(events: Vec<CodexResult<ResponseEvent>>) -> ResponseStream {
    let (tx_event, rx_event) = mpsc::channel(events.len().max(1));
    for event in events {
        tx_event.try_send(event).expect("test stream capacity");
    }
    drop(tx_event);
    ResponseStream {
        rx_event,
        consumer_dropped: CancellationToken::new(),
    }
}

async fn assert_invalid_compaction_charges_budget(count: usize) {
    let (sess, turn_context) = budget_session(100).await;
    let result =
        collect_compaction_output(&sess, &turn_context, completed_stream(count, Some(usage())))
            .await;
    let error = result.err().expect("invalid compaction output");
    assert!(matches!(error.details(), CodexErrorDetails::Fatal(_)));
    assert_eq!(remaining_budget(&sess), 85, "output item count {count}");
}

#[tokio::test]
async fn completed_compaction_charges_budget_when_output_is_missing() {
    assert_invalid_compaction_charges_budget(0).await;
}

#[tokio::test]
async fn completed_compaction_charges_budget_when_output_is_duplicated() {
    assert_invalid_compaction_charges_budget(2).await;
}

#[tokio::test]
async fn completed_compaction_charges_budget_once_for_valid_output() {
    let (sess, turn_context) = budget_session(100).await;
    let result =
        collect_compaction_output(&sess, &turn_context, completed_stream(1, Some(usage()))).await;
    assert!(result.is_ok());
    assert_eq!(remaining_budget(&sess), 85);
}

#[tokio::test]
async fn completed_compaction_enforces_budget_before_output_validation() {
    for count in [0, 1, 2] {
        let (sess, turn_context) = budget_session(15).await;
        let result =
            collect_compaction_output(&sess, &turn_context, completed_stream(count, Some(usage())))
                .await;
        let error = result.err().expect("exhausted budget");
        assert!(matches!(
            error.details(),
            CodexErrorDetails::SessionBudgetExceeded
        ));
        assert_eq!(remaining_budget(&sess), 0);
    }
}

#[tokio::test]
async fn completed_compaction_uses_server_budget_units() {
    let (sess, turn_context) = budget_session(100).await;
    let mut usage = usage();
    usage.codex_rollout_budget_units = Some(serde_json::Number::from(7));
    let result =
        collect_compaction_output(&sess, &turn_context, completed_stream(0, Some(usage))).await;
    assert!(result.is_err());
    assert_eq!(remaining_budget(&sess), 93);
}

#[tokio::test]
async fn compaction_without_usage_leaves_budget_unchanged() {
    let (sess, turn_context) = budget_session(100).await;
    let result = collect_compaction_output(&sess, &turn_context, completed_stream(1, None)).await;
    assert!(result.is_ok());
    assert_eq!(remaining_budget(&sess), 100);
    let result = collect_compaction_output(
        &sess,
        &turn_context,
        response_stream(vec![Err(CodexErr::Stream("disconnected".to_string()))]),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(remaining_budget(&sess), 100);
}
