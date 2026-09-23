//! Abandoned hidden turns must stop their model requests before being detached.

use super::STRUCTURED_RESPONSE_MAX_BYTES;
use super::TemporaryStructuredThreadOptions;
use super::run_temporary_structured_turn;
use super::start_temporary_thread;
use crate::test_support::PathBufExt;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::TurnStatus;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug)]
enum Abandonment {
    Deadline,
    ClosedNotifications,
    OversizedResponse,
}

async fn check_abandoned_turn(abandonment: Abandonment) -> color_eyre::Result<()> {
    let server = wiremock::MockServer::start().await;
    let response = responses::mount_response_once(
        &server,
        responses::sse_response(responses::sse(vec![
            responses::ev_response_created("hidden-response"),
            responses::ev_assistant_message("hidden-message", "unfinished"),
            responses::ev_completed("hidden-response"),
        ]))
        .set_delay(Duration::from_secs(90)),
    )
    .await;
    let (chat_widget, _, _, _) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    let mut config = chat_widget.config_ref().clone();
    let home = tempfile::tempdir()?;
    let provider = "hidden-cleanup-test";
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.2\"\nmodel_provider = \"{provider}\"\n\
             [model_providers.{provider}]\nname = \"Hidden cleanup test\"\n\
             base_url = \"{}/v1\"\nwire_api = \"responses\"\n\
             request_max_retries = 0\nstream_max_retries = 0\n",
            server.uri()
        ),
    )?;
    config.codex_home = home.path().to_path_buf().abs();
    config.sqlite = codex_state::SqliteConfig::new_for_testing(config.codex_home.clone());
    config.model = Some("gpt-5.2".to_string());
    config.model_provider_id = provider.to_string();
    config.model_provider = ModelProviderInfo {
        name: "Hidden cleanup test".to_string(),
        base_url: Some(format!("{}/v1", server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };
    let mut app_server = crate::start_embedded_app_server_for_picker(&config).await?;
    let started = start_temporary_thread(
        &app_server.request_handle(),
        TemporaryStructuredThreadOptions {
            thread_source: ThreadSource::Feature("thread_title".to_string()),
            model: "gpt-5.2".to_string(),
            model_provider: provider.to_string(),
            cwd: config.cwd.display().to_string(),
            active_permission_profile: None,
            mcp_server_names: Vec::new(),
        },
    )
    .await?;
    let thread_id = started.thread.id;
    let (sender, receiver) = unbounded_channel();
    let mut request = tokio::spawn(run_temporary_structured_turn(
        app_server.request_handle(),
        thread_id.clone(),
        "Produce a short title".to_string(),
        serde_json::json!({"type":"object","properties":{"title":{"type":"string"}},"required":["title"],"additionalProperties":false}),
        None,
        receiver,
        CancellationToken::new(),
    ));
    let turn_id = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(AppServerEvent::ServerNotification(notification)) =
                app_server.next_event().await
                && let ServerNotification::TurnStarted(started) = notification.as_ref()
                && started.thread_id == thread_id
            {
                break started.turn.id.clone();
            }
        }
    })
    .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while response.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    match abandonment {
        Abandonment::Deadline => {}
        Abandonment::ClosedNotifications => drop(sender),
        Abandonment::OversizedResponse => {
            sender.send(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    item: ThreadItem::AgentMessage {
                        id: "oversized".to_string(),
                        text: "x".repeat(STRUCTURED_RESPONSE_MAX_BYTES + 1),
                        phase: None,
                        memory_citation: None,
                        delivery: None,
                        questions: None,
                    },
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    completed_at_ms: 0,
                },
            ))?;
        }
    }
    let mut outcome = None;
    let mut interrupted = false;
    let observed = tokio::time::timeout(Duration::from_secs(40), async {
        loop {
            tokio::select! {
                result = &mut request, if outcome.is_none() => {
                    outcome = Some(result.expect("hidden request task"));
                }
                event = app_server.next_event() => {
                    if let Some(AppServerEvent::ServerNotification(notification)) = event
                        && let ServerNotification::TurnCompleted(completed) = notification.as_ref()
                        && completed.thread_id == thread_id
                        && completed.turn.id == turn_id
                    {
                        interrupted = completed.turn.status == TurnStatus::Interrupted;
                    }
                }
            }
            if outcome.is_some() && interrupted {
                break;
            }
        }
    })
    .await;
    // Always close the fixture, including when the original implementation leaks its turn.
    app_server.shutdown().await?;
    assert!(
        observed.is_ok(),
        "abandoned {abandonment:?} turn kept running after its caller returned"
    );
    assert!(outcome.expect("request outcome").is_err());
    assert!(interrupted);
    assert_eq!(response.requests().len(), 1);
    Ok(())
}

#[tokio::test]
async fn deadline_interrupts_hidden_generation() -> color_eyre::Result<()> {
    check_abandoned_turn(Abandonment::Deadline).await
}

#[tokio::test]
async fn closed_notifications_interrupt_hidden_generation() -> color_eyre::Result<()> {
    check_abandoned_turn(Abandonment::ClosedNotifications).await
}

#[tokio::test]
async fn oversized_response_interrupts_hidden_generation() -> color_eyre::Result<()> {
    check_abandoned_turn(Abandonment::OversizedResponse).await
}
