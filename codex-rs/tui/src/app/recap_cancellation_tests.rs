//! Cancellation of a recap whose result can no longer be displayed.

use super::RECAP_DELAY;
use super::RecapProgress;
use super::RecapRequest;
use crate::app::test_support::make_test_app;
use crate::app_event::AppEvent;
use crate::app_event::RecapTrigger;
use crate::app_event_sender::AppEventSender;
use crate::temporary_structured_request::TemporaryStructuredThreadOptions;
use crate::temporary_structured_request::start_temporary_thread;
use crate::test_support::PathBufExt;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::TurnStatus;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::ThreadId;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
enum Invalidation {
    FocusRestored,
    AutomaticRecapsDisabled,
    ThreadReplaced,
    NewerTurnFinished,
    ManualFocusRestored,
    ManualAutomaticRecapsDisabled,
}

async fn check_invalidated_recap(invalidation: Invalidation) -> color_eyre::Result<()> {
    let preserves_manual = matches!(
        invalidation,
        Invalidation::ManualFocusRestored | Invalidation::ManualAutomaticRecapsDisabled
    );
    let server = wiremock::MockServer::start().await;
    let response = responses::mount_response_once(
        &server,
        responses::sse_response(responses::sse(vec![
            responses::ev_response_created("recap-response"),
            responses::ev_assistant_message(
                "recap-message",
                r#"{"summary":"old work","next_action":null}"#,
            ),
            responses::ev_completed("recap-response"),
        ]))
        .set_delay(if preserves_manual {
            Duration::from_millis(/*millis*/ 600)
        } else {
            Duration::from_secs(/*secs*/ 60)
        }),
    )
    .await;
    let mut app = make_test_app().await;
    let (ui_tx, mut ui_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(ui_tx);
    let home = tempfile::tempdir()?;
    let provider = "recap-cancellation-test";
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.2\"\nmodel_provider = \"{provider}\"\n\
        [model_providers.{provider}]\nname = \"Recap cancellation test\"\n\
        base_url = \"{}/v1\"\nwire_api = \"responses\"\n\
        request_max_retries = 0\nstream_max_retries = 0\n",
            server.uri()
        ),
    )?;
    let mut config = app.chat_widget.config_ref().clone();
    config.codex_home = home.path().to_path_buf().abs();
    config.sqlite = codex_state::SqliteConfig::new_for_testing(config.codex_home.clone());
    config.model = Some("gpt-5.2".to_string());
    config.model_provider_id = provider.to_string();
    config.model_provider = ModelProviderInfo {
        name: "Recap cancellation test".to_string(),
        base_url: Some(format!("{}/v1", server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };
    let mut app_server = crate::start_embedded_app_server_for_picker(&config).await?;
    let hidden = start_temporary_thread(
        &app_server.request_handle(),
        TemporaryStructuredThreadOptions {
            thread_source: ThreadSource::Feature("system".to_string()),
            model: "gpt-5.2".to_string(),
            model_provider: provider.to_string(),
            cwd: config.cwd.display().to_string(),
            active_permission_profile: None,
            mcp_server_names: Vec::new(),
        },
    )
    .await?;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.local_settings.tui.auto_recap = true;
    let ready_since = Instant::now();
    app.recap.note_focus_lost(ready_since);
    app.recap.seed_from_progress(
        RecapProgress {
            completed_turns: 3,
            last_recapped_turn_count: None,
        },
        ready_since,
    );
    // Advance the scheduler clock without subtracting from a freshly booted host.
    tokio::time::pause();
    tokio::time::advance(RECAP_DELAY + Duration::from_secs(/*secs*/ 1)).await;
    tokio::time::resume();
    let trigger = match invalidation {
        Invalidation::ThreadReplaced
        | Invalidation::ManualFocusRestored
        | Invalidation::ManualAutomaticRecapsDisabled => RecapTrigger::Manual,
        Invalidation::FocusRestored
        | Invalidation::AutomaticRecapsDisabled
        | Invalidation::NewerTurnFinished => RecapTrigger::Automatic,
    };
    let request = RecapRequest {
        thread_id,
        request_id: Uuid::new_v4(),
        trigger,
        completed_turn_count: app.recap.completed_turns,
        turn_revision: app.recap.turn_revision,
    };
    app.recap.in_flight_request_id = Some(request.request_id);
    app.recap.in_flight_trigger = Some(trigger);
    let hidden_id = hidden.thread.id;
    app.handle_recap_started(
        &app_server,
        request,
        "Earlier work".to_string(),
        Ok(hidden_id.clone()),
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        while response.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await?;

    match invalidation {
        Invalidation::FocusRestored | Invalidation::ManualFocusRestored => {
            app.recap.note_focus_gained()
        }
        Invalidation::AutomaticRecapsDisabled | Invalidation::ManualAutomaticRecapsDisabled => {
            app.local_settings.tui.auto_recap = false;
            app.schedule_recap_check(thread_id, Instant::now());
        }
        Invalidation::ThreadReplaced => app.recap.reset_for_new_thread(Instant::now()),
        Invalidation::NewerTurnFinished => app
            .recap
            .note_turn_finished(&TurnStatus::Completed, Instant::now()),
    }
    let stopped = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        let mut status = None;
        let mut worker_result = None;
        loop {
            tokio::select! {
                event = app_server.next_event() => {
                    let event = event.ok_or_else(|| color_eyre::eyre::eyre!("embedded app-server event stream closed"))?;
                    if let AppServerEvent::ServerNotification(notification) = &event
                        && let ServerNotification::TurnCompleted(completed) = notification.as_ref()
                        && completed.thread_id == hidden_id
                    {
                        status = Some(completed.turn.status.clone());
                    }
                    app.handle_app_server_event(&app_server, event).await;
                }
                event = ui_rx.recv() => {
                    if let Some(AppEvent::RecapGenerated { result, .. }) = event {
                        worker_result = Some(result);
                    }
                }
            }
            if let (Some(status), Some(result)) = (&status, &worker_result) {
                assert_eq!(result.is_ok(), preserves_manual, "{invalidation:?}");
                return Ok::<_, color_eyre::Report>(status.clone());
            }
        }
    }).await;
    // Close the actual model request even when the baseline fails this assertion.
    app_server.shutdown().await?;
    let expected = if preserves_manual {
        TurnStatus::Completed
    } else {
        TurnStatus::Interrupted
    };
    assert_eq!(
        stopped.ok().transpose()?,
        Some(expected),
        "obsolete recap kept sampling after {invalidation:?}"
    );
    assert_eq!(response.requests().len(), 1);
    Ok(())
}

#[tokio::test]
async fn regaining_focus_interrupts_in_flight_automatic_recap() -> color_eyre::Result<()> {
    check_invalidated_recap(Invalidation::FocusRestored).await
}

#[tokio::test]
async fn disabling_automatic_recaps_interrupts_in_flight_generation() -> color_eyre::Result<()> {
    check_invalidated_recap(Invalidation::AutomaticRecapsDisabled).await
}

#[tokio::test]
async fn replacing_thread_interrupts_its_in_flight_manual_recap() -> color_eyre::Result<()> {
    check_invalidated_recap(Invalidation::ThreadReplaced).await
}

#[tokio::test]
async fn newer_turn_completion_interrupts_obsolete_recap() -> color_eyre::Result<()> {
    check_invalidated_recap(Invalidation::NewerTurnFinished).await
}

#[tokio::test]
async fn regaining_focus_preserves_in_flight_manual_recap() -> color_eyre::Result<()> {
    check_invalidated_recap(Invalidation::ManualFocusRestored).await
}

#[tokio::test]
async fn disabling_automatic_recaps_preserves_in_flight_manual_recap() -> color_eyre::Result<()> {
    check_invalidated_recap(Invalidation::ManualAutomaticRecapsDisabled).await
}
