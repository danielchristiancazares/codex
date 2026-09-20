//! An admitted hidden turn still needs interruption when its start acknowledgement is lost.

use super::TemporaryStructuredThreadOptions;
use super::run_temporary_structured_turn;
use super::start_temporary_thread;
use crate::test_support::PathBufExt;
use codex_app_server_client::AppServerEvent;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::TurnStatus;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn lost_start_acknowledgement_does_not_orphan_hidden_inference() -> color_eyre::Result<()> {
    let model_server = wiremock::MockServer::start().await;
    let model = responses::mount_response_once(
        &model_server,
        responses::sse_response(responses::sse(vec![
            responses::ev_response_created("orphan-response"),
            responses::ev_assistant_message("orphan-message", "still generating"),
            responses::ev_completed("orphan-response"),
        ]))
        .set_delay(Duration::from_secs(/*secs*/ 90)),
    )
    .await;
    let (chat_widget, _, _, _) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    let mut config = chat_widget.config_ref().clone();
    let home = tempfile::tempdir()?;
    let provider = "missing-ack-test";
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.2\"\nmodel_provider = \"{provider}\"\n\
        [model_providers.{provider}]\nname = \"Missing acknowledgement test\"\n\
        base_url = \"{}/v1\"\nwire_api = \"responses\"\n\
        request_max_retries = 0\nstream_max_retries = 0\n",
            model_server.uri()
        ),
    )?;
    config.codex_home = home.path().to_path_buf().abs();
    config.sqlite = codex_state::SqliteConfig::new_for_testing(config.codex_home.clone());
    config.model = Some("gpt-5.2".to_string());
    config.model_provider_id = provider.to_string();
    config.model_provider = ModelProviderInfo {
        name: "Missing acknowledgement test".to_string(),
        base_url: Some(format!("{}/v1", model_server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };
    let mut embedded = crate::start_embedded_app_server_for_picker(&config).await?;
    let started = start_temporary_thread(
        &embedded.request_handle(),
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
    let embedded_requests = embedded.request_handle();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let (observed_tx, mut observed_rx) = tokio::sync::mpsc::unbounded_channel();
    let proxy = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let mut websocket = tokio_tungstenite::accept_async(socket).await?;
        loop {
            tokio::select! {
                frame = websocket.next() => {
                    let Some(frame) = frame else { break; };
                    let tokio_tungstenite::tungstenite::Message::Text(text) = frame? else { break; };
                    let message = serde_json::from_str::<JSONRPCMessage>(&text)?;
                    if let JSONRPCMessage::Request(request) = message {
                        let id = request.id.clone();
                        let method = request.method.clone();
                        let result = if method == "initialize" {
                            serde_json::json!({"userAgent":"hidden-turn-ack-test"})
                        } else {
                            let typed: ClientRequest = serde_json::from_value(serde_json::to_value(request)?)?;
                            embedded_requests.request(typed).await?.map_err(|error| color_eyre::eyre::eyre!("{}", error.message))?
                        };
                        observed_tx.send(method.clone())?;
                        if method != "turn/start" {
                            websocket.send(tokio_tungstenite::tungstenite::Message::Text(
                                serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse { id, result }))?.into()
                            )).await?;
                        }
                    }
                }
                event = embedded.next_event() => {
                    let Some(event) = event else { break; };
                    if let AppServerEvent::ServerNotification(notification) = event {
                        websocket.send(tokio_tungstenite::tungstenite::Message::Text(
                            serde_json::to_string(&notification)?.into()
                        )).await?;
                    }
                }
            }
        }
        embedded.shutdown().await?;
        Ok::<(), color_eyre::Report>(())
    });
    let mut remote = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
        endpoint: RemoteAppServerEndpoint::WebSocket {
            websocket_url,
            auth_token: None,
        },
        client_name: "hidden-turn-ack-test".to_string(),
        client_version: "1".to_string(),
        experimental_api: true,
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: 32,
    })
    .await?;
    let (notifications_tx, notifications_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut operation = tokio::spawn(run_temporary_structured_turn(
        AppServerRequestHandle::Remote(remote.request_handle()),
        thread_id.clone(),
        "Make a title".to_string(),
        serde_json::json!({"type":"string"}),
        /*effort*/ None,
        notifications_rx,
        CancellationToken::new(),
    ));
    let mut result = None;
    let mut interrupted = false;
    let mut admitted = false;
    let observed = tokio::time::timeout(Duration::from_secs(/*secs*/ 35), async {
        loop {
            tokio::select! {
                outcome = &mut operation, if result.is_none() => result = Some(outcome.expect("hidden worker")),
                event = remote.next_event() => {
                    let Some(event) = event else { break; };
                    if let AppServerEvent::ServerNotification(notification) = event {
                        if let ServerNotification::TurnStarted(started) = notification.as_ref()
                            && started.thread_id == thread_id {
                            admitted = true;
                            let mut unrelated = started.clone();
                            unrelated.thread_id = "unrelated-hidden-thread".to_string();
                            unrelated.turn.id = "unrelated-turn".to_string();
                            notifications_tx.send(ServerNotification::TurnStarted(unrelated)).expect("unrelated admission fixture");
                        }
                        if let ServerNotification::TurnCompleted(completed) = notification.as_ref()
                            && completed.thread_id == thread_id {
                            interrupted = completed.turn.status == TurnStatus::Interrupted;
                        }
                        let _ = notifications_tx.send(*notification);
                    }
                }
            }
            if result.is_some() && interrupted { break; }
        }
    }).await;
    remote.shutdown().await?;
    proxy.await??;
    let methods = std::iter::from_fn(|| observed_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(admitted, "fixture never admitted the hidden turn");
    assert_eq!(model.requests().len(), 1);
    assert!(result.expect("timed-out worker result").is_err());
    assert!(
        observed.is_ok() && interrupted,
        "missing start acknowledgement orphaned an admitted model request; methods {methods:?}"
    );
    assert_eq!(
        methods
            .iter()
            .filter(|method| *method == "turn/interrupt")
            .count(),
        1
    );
    Ok(())
}
