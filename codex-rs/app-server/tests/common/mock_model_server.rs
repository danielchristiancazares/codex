use anyhow::Context;
use anyhow::Result;
use core_test_support::responses::WebSocketTestServer;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use core_test_support::responses;
use serde_json::Value;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

/// Create a mock server that will provide the responses, in order, for
/// requests to the `/v1/responses` endpoint.
pub async fn create_mock_responses_server_sequence(responses: Vec<String>) -> MockServer {
    let server = responses::start_mock_server().await;

    let num_calls = responses.len();
    let seq_responder = SeqResponder {
        num_calls: AtomicUsize::new(0),
        responses,
    };

    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(seq_responder)
        .expect(num_calls as u64)
        .mount(&server)
        .await;

    server
}

/// Same as `create_mock_responses_server_sequence` but does not enforce an
/// expectation on the number of calls.
pub async fn create_mock_responses_server_sequence_unchecked(responses: Vec<String>) -> MockServer {
    let server = responses::start_mock_server().await;

    let seq_responder = SeqResponder {
        num_calls: AtomicUsize::new(0),
        responses,
    };

    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(seq_responder)
        .mount(&server)
        .await;

    server
}

struct SeqResponder {
    num_calls: AtomicUsize,
    responses: Vec<String>,
}

impl Respond for SeqResponder {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        let call_num = self.num_calls.fetch_add(1, Ordering::SeqCst);
        let response = self
            .responses
            .get(call_num)
            .expect("mock model response should exist");
        responses::sse_response(response.clone())
    }
}

/// Create a mock responses API server that returns the same assistant message for every request.
pub async fn create_mock_responses_server_repeating_assistant(message: &str) -> MockServer {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", message),
        responses::ev_completed("resp-1"),
    ]);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responses::sse_response(body))
        .mount(&server)
        .await;
    server
}

/// Starts a WebSocket Responses fixture from the existing SSE response bodies.
///
/// The first request is reserved for the client's connection warmup, matching
/// the production WebSocket request lifecycle.
pub async fn create_mock_responses_websocket_server_sequence(
    responses: Vec<String>,
) -> Result<WebSocketTestServer> {
    create_mock_responses_websocket_server_connections(vec![responses]).await
}

/// Starts a WebSocket Responses fixture with one scripted request sequence per connection.
pub async fn create_mock_responses_websocket_server_connections(
    connections: Vec<Vec<String>>,
) -> Result<WebSocketTestServer> {
    let connections = connections
        .into_iter()
        .map(|response_bodies| {
            let mut requests = vec![vec![
                responses::ev_response_created("prewarm"),
                responses::ev_completed("prewarm"),
            ]];
            requests.extend(
                response_bodies
                    .into_iter()
                    .map(|response| parse_sse_events(&response))
                    .collect::<Result<Vec<_>>>()?,
            );
            Ok(requests)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(responses::start_websocket_server(connections).await)
}

pub fn websocket_model_request_bodies(server: &WebSocketTestServer) -> Vec<Value> {
    server
        .connections()
        .into_iter()
        .flatten()
        .map(|request| request.body_json())
        .filter(|body| body.get("generate") != Some(&Value::Bool(false)))
        .collect()
}

pub fn response_request_input(body: &Value) -> Vec<Value> {
    body["input"]
        .as_array()
        .expect("input array not found in request")
        .clone()
}

pub fn response_request_message_input_texts(body: &Value, role: &str) -> Vec<String> {
    response_request_input(body)
        .into_iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter(|item| item.get("role").and_then(Value::as_str) == Some(role))
        .filter_map(|item| item.get("content").and_then(Value::as_array).cloned())
        .flatten()
        .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

pub fn response_request_function_call_output_text(body: &Value, call_id: &str) -> Option<String> {
    let output = response_request_input(body)
        .into_iter()
        .find(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })?
        .get("output")
        .cloned()?;
    match output {
        Value::String(text) => Some(text),
        Value::Array(items) => match items.as_slice() {
            [item] if item.get("type").and_then(Value::as_str) == Some("input_text") => {
                item.get("text").and_then(Value::as_str).map(str::to_owned)
            }
            [] | [_] | [_, _, ..] => None,
        },
        Value::Object(object) => object
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

pub fn sse_response_events(response: &str) -> Result<Vec<Value>> {
    parse_sse_events(response)
}

fn parse_sse_events(response: &str) -> Result<Vec<Value>> {
    response
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .map(|block| {
            let mut event_type = None;
            let mut data = None;
            for line in block.lines() {
                if let Some(value) = line.strip_prefix("event: ") {
                    event_type = Some(value);
                } else if let Some(value) = line.strip_prefix("data: ") {
                    data = Some(value);
                }
            }
            match data {
                Some(data) => serde_json::from_str(data).context("parse SSE response event"),
                None => Ok(serde_json::json!({
                    "type": event_type.context("SSE event is missing its type")?,
                })),
            }
        })
        .collect()
}
