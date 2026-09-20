use super::ResponsesStreamRequest;
use super::log_retry;
use crate::session::tests::make_session_and_context;
use codex_protocol::error::CodexErr;
use std::time::Duration;
use tracing_test::internal::MockWriter;

#[tokio::test]
async fn sampling_retry_logs_stream_error_context() {
    let (_session, turn_context) = make_session_and_context().await;
    let buffer: &'static std::sync::Mutex<Vec<u8>> =
        Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);

    log_retry(
        ResponsesStreamRequest::Sampling,
        &turn_context,
        &CodexErr::Stream("websocket closed by server before response.completed".to_string()),
        /*retries*/ 2,
        /*max_retries*/ 5,
        Duration::from_secs(1),
    );

    let logs = String::from_utf8(
        buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
    )
    .expect("retry log should be valid utf-8");
    assert!(logs.contains("stream disconnected - retrying sampling request"));
    assert!(logs.contains(&format!("turn_id={}", turn_context.sub_id)));
    assert!(logs.contains("retries=2"));
    assert!(logs.contains("max_retries=5"));
    assert!(logs.contains(
        "sampling_error=stream disconnected before completion: websocket closed by server before response.completed"
    ));
}

#[test_case::test_case(ResponsesStreamRequest::Sampling; "sampling")]
#[test_case::test_case(ResponsesStreamRequest::RemoteCompactionV2; "compaction")]
#[tokio::test]
async fn transport_fallback_respects_server_retry_delay(request: ResponsesStreamRequest) {
    let (session, turn_context, _events) =
        crate::session::tests::make_session_and_context_with_auth_and_config_and_rx(
            codex_login::CodexAuth::from_api_key("Test API Key"),
            Vec::new(),
            |config| config.model_provider.supports_websockets = true,
        )
        .await;
    let mut client_session = session.services.model_client.new_session();
    let mut retry_state = super::ResponsesStreamRetryState {
        retries: 1,
        ..Default::default()
    };
    let delay = Duration::from_secs(5);
    let error = CodexErr::Stream("slow down".to_string()).with_retry_delay(delay);
    tokio::time::pause();
    let started = tokio::time::Instant::now();

    super::handle_response_stream_error(
        &mut retry_state,
        /*max_retries*/ 1,
        error,
        &mut client_session,
        &session,
        &turn_context,
        request,
    )
    .await
    .expect("fallback should allow a request after the advised delay");

    assert!(
        started.elapsed() >= delay,
        "fallback must wait at least the advised delay"
    );
    pretty_assertions::assert_eq!(retry_state.retries, 0);
    assert!(!session.services.model_client.responses_websocket_enabled());
}

#[test_case::test_case(None; "no_advice")]
#[test_case::test_case(Some(Duration::ZERO); "zero_advice")]
#[tokio::test]
async fn transport_fallback_without_positive_advice_is_immediate(advice: Option<Duration>) {
    let (session, turn_context, _events) =
        crate::session::tests::make_session_and_context_with_auth_and_config_and_rx(
            codex_login::CodexAuth::from_api_key("Test API Key"),
            Vec::new(),
            |config| config.model_provider.supports_websockets = true,
        )
        .await;
    let mut client_session = session.services.model_client.new_session();
    let mut retry_state = super::ResponsesStreamRetryState::default();
    let error = CodexErr::Stream("connection closed".to_string());
    let error = match advice {
        Some(delay) => error.with_retry_delay(delay),
        None => error,
    };
    tokio::time::pause();
    let started = tokio::time::Instant::now();

    super::handle_response_stream_error(
        &mut retry_state,
        /*max_retries*/ 0,
        error,
        &mut client_session,
        &session,
        &turn_context,
        ResponsesStreamRequest::Sampling,
    )
    .await
    .expect("fallback should recover immediately without positive server advice");

    pretty_assertions::assert_eq!(started.elapsed(), Duration::ZERO);
    pretty_assertions::assert_eq!(retry_state.retries, 0);
    assert!(!session.services.model_client.responses_websocket_enabled());
}

#[tokio::test]
async fn terminal_error_with_server_advice_never_switches_transport() {
    let (session, turn_context, _events) =
        crate::session::tests::make_session_and_context_with_auth_and_config_and_rx(
            codex_login::CodexAuth::from_api_key("Test API Key"),
            Vec::new(),
            |config| config.model_provider.supports_websockets = true,
        )
        .await;
    let mut client_session = session.services.model_client.new_session();
    let mut retry_state = super::ResponsesStreamRetryState::default();
    tokio::time::pause();
    let started = tokio::time::Instant::now();

    let error = super::handle_response_stream_error(
        &mut retry_state,
        /*max_retries*/ 0,
        CodexErr::QuotaExceeded.with_retry_delay(Duration::from_secs(5)),
        &mut client_session,
        &session,
        &turn_context,
        ResponsesStreamRequest::Sampling,
    )
    .await
    .expect_err("terminal quota errors must not retry");

    assert!(matches!(
        error.details(),
        codex_protocol::error::CodexErrorDetails::QuotaExceeded
    ));
    pretty_assertions::assert_eq!(started.elapsed(), Duration::ZERO);
    assert!(session.services.model_client.responses_websocket_enabled());
}
