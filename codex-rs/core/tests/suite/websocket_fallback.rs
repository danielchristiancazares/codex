use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_model_provider_info::WireApi;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::TempDirExt;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use pretty_assertions::assert_eq;
use tokio::time::Duration;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::http::Method;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_fallback_switches_to_http_on_upgrade_required_connect() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/responses$"))
        .respond_with(ResponseTemplate::new(426))
        .mount(&server)
        .await;

    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            // If we don't treat 426 specially, the sampling loop would retry the WebSocket
            // handshake before switching to the HTTP transport.
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn("hello").await?;

    let requests = server.received_requests().await.unwrap_or_default();
    let websocket_attempts = requests
        .iter()
        .filter(|req| req.method == Method::GET && req.url.path().ends_with("/responses"))
        .count();
    let http_attempts = requests
        .iter()
        .filter(|req| req.method == Method::POST && req.url.path().ends_with("/responses"))
        .count();

    // The startup prewarm request sees 426 and immediately switches the session to HTTP fallback,
    // so the first turn goes straight to HTTP with no additional websocket connect attempt.
    assert_eq!(websocket_attempts, 1);
    assert_eq!(http_attempts, 1);
    assert_eq!(response_mock.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_fallback_switches_to_http_after_retries_exhausted() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn("hello").await?;

    let requests = server.received_requests().await.unwrap_or_default();
    let websocket_attempts = requests
        .iter()
        .filter(|req| req.method == Method::GET && req.url.path().ends_with("/responses"))
        .count();
    let http_attempts = requests
        .iter()
        .filter(|req| req.method == Method::POST && req.url.path().ends_with("/responses"))
        .count();

    // Deferred request prewarm is attempted at startup.
    // The first turn then makes 3 websocket stream attempts (initial try + 2 retries),
    // after which fallback activates and the request is replayed over HTTP.
    assert_eq!(websocket_attempts, 4);
    assert_eq!(http_attempts, 1);
    assert_eq!(response_mock.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_fallback_hides_first_websocket_retry_stream_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        }
    });
    let TestCodex {
        codex,
        session_configured,
        cwd,
        ..
    } = builder.build(&server).await?;
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.path());

    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd.abs())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    let mut stream_error_messages = Vec::new();
    loop {
        let event = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .expect("timeout waiting for event")
            .expect("event stream ended unexpectedly")
            .msg;
        match event {
            EventMsg::StreamError(e) => stream_error_messages.push(e.message),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let expected_stream_errors = if cfg!(debug_assertions) {
        vec!["Reconnecting... 1/2", "Reconnecting... 2/2"]
    } else {
        vec!["Reconnecting... 2/2"]
    };
    assert_eq!(stream_error_messages, expected_stream_errors);
    assert_eq!(response_mock.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_fallback_is_sticky_across_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn("first").await?;
    test.submit_turn("second").await?;

    let requests = server.received_requests().await.unwrap_or_default();
    let websocket_attempts = requests
        .iter()
        .filter(|req| req.method == Method::GET && req.url.path().ends_with("/responses"))
        .count();
    let http_attempts = requests
        .iter()
        .filter(|req| req.method == Method::POST && req.url.path().ends_with("/responses"))
        .count();

    // WebSocket attempts all happen on the first turn:
    // 1 deferred request prewarm attempt (startup) + 3 stream attempts
    // (initial try + 2 retries) before fallback.
    // Fallback is sticky, so the second turn stays on HTTP and adds no websocket attempts.
    assert_eq!(websocket_attempts, 4);
    assert_eq!(http_attempts, 2);
    assert_eq!(response_mock.requests().len(), 2);

    Ok(())
}

#[derive(Clone, Copy)]
enum CooldownTurnAction {
    Complete,
    Interrupt,
}

#[test_case::test_case(CooldownTurnAction::Complete; "completes_without_a_premature_request")]
#[test_case::test_case(CooldownTurnAction::Interrupt; "interrupts_without_an_http_request")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_fallback_obeys_server_cooldown(action: CooldownTurnAction) -> Result<()> {
    use futures::SinkExt;
    use futures::StreamExt;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Instant;
    use tokio_tungstenite::tungstenite::Message;

    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let cooldown_until = Arc::new(Mutex::new(None::<Instant>));
    let premature_requests = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with({
            let cooldown_until = Arc::clone(&cooldown_until);
            let premature_requests = Arc::clone(&premature_requests);
            move |_: &wiremock::Request| {
                let deadline = cooldown_until
                    .lock()
                    .expect("cooldown lock")
                    .expect("WebSocket must advertise the cooldown first");
                if Instant::now() < deadline {
                    premature_requests.fetch_add(1, Ordering::SeqCst);
                    responses::sse_response(responses::sse_failed(
                        "premature-http",
                        "slow_down",
                        "Rate limit exceeded. Please try again in 1s.",
                    ))
                } else {
                    responses::sse_response(sse(vec![
                        ev_response_created("http-recovered"),
                        ev_completed("http-recovered"),
                    ]))
                }
            }
        })
        .mount(&server)
        .await;

    // Serve WebSockets and proxy HTTP to the request-recording mock at one endpoint.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let http_address = *server.address();
    let websocket_requests = Arc::new(AtomicUsize::new(0));
    let proxy = tokio_util::task::AbortOnDropHandle::new(tokio::spawn({
        let websocket_requests = Arc::clone(&websocket_requests);
        async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (mut socket, _) = accepted.expect("accept test connection");
                        let websocket_requests = Arc::clone(&websocket_requests);
                        let cooldown_until = Arc::clone(&cooldown_until);
                        connections.spawn(async move {
                            let mut method = [0_u8; 1];
                            if socket.peek(&mut method).await? == 0 {
                                return Ok::<(), anyhow::Error>(());
                            }
                            if method[0] != b'G' {
                                let mut upstream = tokio::net::TcpStream::connect(http_address).await?;
                                tokio::io::copy_bidirectional(&mut socket, &mut upstream).await?;
                                return Ok(());
                            }
                            let mut extensions = tokio_tungstenite::tungstenite::extensions::ExtensionsConfig::default();
                            extensions.permessage_deflate = Some(tokio_tungstenite::tungstenite::extensions::compression::deflate::DeflateConfig::default());
                            let mut websocket_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
                            websocket_config.extensions = extensions;
                            let mut websocket = tokio_tungstenite::accept_async_with_config(socket, Some(websocket_config)).await?;
                            while let Some(message) = websocket.next().await {
                                match message? {
                                    Message::Text(_) | Message::Binary(_) => {}
                                    Message::Ping(payload) => {
                                        websocket.send(Message::Pong(payload)).await?;
                                        continue;
                                    }
                                    Message::Close(_) => break,
                                    _ => continue,
                                }
                                let attempt = websocket_requests.fetch_add(1, Ordering::SeqCst);
                                let events = match attempt {
                                    0 => vec![ev_response_created("prewarm"), ev_completed("prewarm")],
                                    1 | 2 => {
                                        let seconds = if attempt == 1 { 0 } else { 1 };
                                        if attempt == 2 {
                                            *cooldown_until.lock().expect("cooldown lock") =
                                                Some(Instant::now() + Duration::from_secs(1));
                                        }
                                        vec![serde_json::json!({
                                            "type": "response.failed",
                                            "response": {
                                                "id": format!("limited-{attempt}"),
                                                "error": {
                                                    "code": "slow_down",
                                                    "message": format!("Rate limit exceeded. Please try again in {seconds}s.")
                                                }
                                            }
                                        })]
                                    }
                                    _ => anyhow::bail!("unexpected WebSocket request {attempt}"),
                                };
                                for event in events {
                                    websocket.send(Message::Text(event.to_string().into())).await?;
                                }
                                if attempt > 0 {
                                    let _ = websocket.close(None).await;
                                    break;
                                }
                            }
                            Ok(())
                        });
                    }
                    result = connections.join_next(), if !connections.is_empty() => {
                        result.expect("connection result").expect("connection task").expect("test transport");
                    }
                }
            }
        }
    }));

    let test = test_codex()
        .with_config(move |config| {
            config.model_provider.base_url = Some(format!("http://{address}/v1"));
            config
                .features
                .disable(codex_features::Feature::UnboundedConnectionRetries)
                .expect("disable unbounded fixture connection retries");
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = Some(1);
            config.model_provider.request_max_retries = Some(0);
        })
        .build_with_auto_env(&server)
        .await?;

    match action {
        CooldownTurnAction::Complete => {
            timeout(
                Duration::from_secs(20),
                test.submit_turn("recover after the provider cooldown"),
            )
            .await??;
            assert_eq!(
                (
                    premature_requests.load(Ordering::SeqCst),
                    server.received_requests().await.unwrap_or_default().len()
                ),
                (0, 1),
            );
        }
        CooldownTurnAction::Interrupt => {
            test.codex
                .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "interrupt during the provider cooldown".into(),
                    text_elements: Vec::new(),
                }]))
                .await?;
            core_test_support::wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::Warning(warning) if warning.message.starts_with("Falling back from WebSockets"))
            }).await;
            test.codex
                .submit(codex_protocol::protocol::Op::Interrupt)
                .await?;
            core_test_support::wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnAborted(_))
            })
            .await;
            tokio::time::sleep(Duration::from_millis(1100)).await;
            assert_eq!(
                server.received_requests().await.unwrap_or_default().len(),
                0
            );
        }
    }
    assert_eq!(websocket_requests.load(Ordering::SeqCst), 3);
    proxy.abort();
    let server_error = proxy.await.expect_err("test listener was stopped");
    assert!(
        server_error.is_cancelled(),
        "test transport panicked: {server_error}"
    );
    Ok(())
}
