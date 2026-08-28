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
use serde_json::json;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_failed_without_error_reconnects_with_full_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![
            vec![ev_response_created("warmup"), ev_completed("warmup")],
            vec![
                ev_response_created("resp-null-error"),
                json!({
                    "type": "response.failed",
                    "response": {
                        "id": "resp-null-error",
                        "status": "failed",
                        "error": null
                    }
                }),
            ],
        ],
        vec![vec![
            ev_response_created("resp-retry"),
            ev_completed("resp-retry"),
        ]],
    ])
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.model_provider.stream_max_retries = 0.into();
    });
    let test = builder.build_with_websocket_server(&server).await?;

    test.submit_turn("hello").await?;

    assert_eq!(server.handshakes().len(), 2);
    let connections = server.connections();
    assert_eq!(
        connections.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2, 1]
    );
    let failed_request = connections[0][1].body_json();
    let retry_request = connections[1][0].body_json();
    assert_eq!(retry_request.get("previous_response_id"), None);
    assert_eq!(retry_request["input"], failed_request["input"]);
    assert!(
        retry_request["input"]
            .as_array()
            .is_some_and(|input| !input.is_empty())
    );

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_response_is_terminal_with_unbounded_websocket_retries() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![
            vec![ev_response_created("warmup"), ev_completed("warmup")],
            vec![
                ev_response_created("resp-incomplete"),
                json!({
                    "type": "response.incomplete",
                    "response": {
                        "id": "resp-incomplete",
                        "incomplete_details": { "reason": "max_output_tokens" },
                        "usage": {
                            "input_tokens": 10,
                            "input_tokens_details": null,
                            "output_tokens": 5,
                            "output_tokens_details": null,
                            "total_tokens": 15
                        }
                    }
                }),
            ],
        ],
        vec![vec![
            ev_response_created("retry-trap"),
            ev_completed("retry-trap"),
        ]],
    ])
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.model_provider.stream_max_retries = 0.into();
    });
    let test = builder.build_with_websocket_server(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "reach the output limit".into(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let error = wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await;
    assert!(matches!(
        error,
        EventMsg::Error(error)
            if error.message == "incomplete response returned, reason: max_output_tokens"
    ));
    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 2);

    server.shutdown().await;
    Ok(())
}
