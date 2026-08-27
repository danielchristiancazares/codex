use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_model_provider_info::WireApi;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::http::Method;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upgrade_required_keeps_retrying_websocket_without_http() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/responses$"))
        .respond_with(ResponseTemplate::new(/*status*/ 426))
        .mount(&server)
        .await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-http"),
            ev_completed("resp-http"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config({
        let base_url = format!("{}/v1", server.uri());
        move |config| {
            config.model_provider.base_url = base_url.into();
            config.model_provider.wire_api = WireApi::Responses;
            config.model_provider.supports_websockets = true;
            config.model_provider.stream_max_retries = 0.into();
            config.model_provider.request_max_retries = 0.into();
        }
    });
    let test = builder.build(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let requests = timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            let requests = server.received_requests().await.unwrap_or_default();
            let websocket_attempts = requests
                .iter()
                .filter(|request| {
                    request.method == Method::GET && request.url.path().ends_with("/responses")
                })
                .count();
            if websocket_attempts >= 3 {
                break requests;
            }
            sleep(Duration::from_millis(/*millis*/ 25)).await;
        }
    })
    .await
    .expect("websocket connection should be retried beyond the configured budget");

    assert_eq!(
        requests
            .iter()
            .filter(|request| {
                request.method == Method::POST && request.url.path().ends_with("/responses")
            })
            .count(),
        0
    );
    assert_eq!(response_mock.requests().len(), 0);

    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_reconnects_until_the_provider_recovers() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![],
        vec![],
        vec![vec![ev_response_created("resp-1"), ev_completed("resp-1")]],
    ])
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.model_provider.stream_max_retries = 0.into();
    });
    let test = builder.build_with_websocket_server(&server).await?;

    test.submit_turn("hello").await?;

    assert_eq!(server.handshakes().len(), 3);
    assert_eq!(
        server
            .connections()
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![0, 0, 1]
    );

    server.shutdown().await;

    Ok(())
}
