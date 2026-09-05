//! Runs existing SSE/body-assertion fixtures behind the fork's WebSocket transport.

use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use codex_http_client::HttpClientBuilder;
use core_test_support::responses;
use futures::SinkExt;
use futures::StreamExt;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::extensions::ExtensionsConfig;
use tokio_tungstenite::tungstenite::extensions::compression::deflate::DeflateConfig;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

/// Owns the HTTP response script and the WebSocket endpoint used by app-server.
pub(super) struct ResponsesWebSocketBridge {
    // Cancel forwarding before dropping the HTTP response script.
    forwarder: ResponsesWebSocketForwarder,
    pub(super) http: wiremock::MockServer,
}

impl ResponsesWebSocketBridge {
    pub(super) async fn start() -> Result<Self> {
        let http = responses::start_mock_server().await;
        let forwarder = ResponsesWebSocketForwarder::start(&http.uri()).await?;
        Ok(Self { http, forwarder })
    }

    pub(super) fn uri(&self) -> &str {
        self.forwarder.uri()
    }
}

/// Adapts a loopback SSE fixture, including gated streams, to production WebSocket requests.
/// Request bodies retain their actual per-request client metadata for assertions.
pub(super) struct ResponsesWebSocketForwarder {
    uri: String,
    bridge: JoinHandle<()>,
}

impl ResponsesWebSocketForwarder {
    pub(super) async fn start(upstream_base: &str) -> Result<Self> {
        let upstream = format!("{upstream_base}/v1/responses");
        // Both endpoints belong to this loopback-only fixture.
        let client = HttpClientBuilder::new()
            .without_redirects()
            .without_request_logging()
            .build_direct()?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let uri = format!("http://{}", listener.local_addr()?);
        let bridge = tokio::spawn(async move {
            let mut connections = JoinSet::<Result<()>>::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.expect("accept fixture WebSocket");
                        let client = client.clone();
                        let upstream = upstream.clone();
                        connections.spawn(async move {
                            // Optional HTTP catalog probes can fail as on the bare mock server.
                            let mut extensions = ExtensionsConfig::default();
                            extensions.permessage_deflate = Some(DeflateConfig::default());
                            let mut config = WebSocketConfig::default();
                            config.extensions = extensions;
                            let Ok(mut socket) = accept_async_with_config(stream, Some(config)).await else {
                                return Ok(());
                            };
                            let mut warmup_sequence = 0usize;
                            let mut response_history = HashMap::<String, Vec<Value>>::new();
                            while let Some(Ok(message)) = socket.next().await {
                                let mut body: Value = match message {
                                    Message::Text(text) => serde_json::from_str(&text)?,
                                    Message::Binary(bytes) => serde_json::from_slice(&bytes)?,
                                    Message::Close(_) => break,
                                    Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                                };
                                // The HTTP fixture validates complete call/output history.
                                // Expand incremental creates locally while preserving socket reuse.
                                let mut logical_history = match body.get("previous_response_id").and_then(Value::as_str) {
                                    Some(id) => response_history.get(id)
                                        .with_context(|| format!("unknown fixture response {id}"))?
                                        .clone(),
                                    None => Vec::new(),
                                };
                                logical_history.extend(body["input"].as_array().context("fixture request input")?.iter().cloned());
                                body["input"] = Value::Array(logical_history.clone());
                                body.as_object_mut().context("fixture request object")?.remove("previous_response_id");
                                let is_warmup = body["generate"] == false;
                                let events = if is_warmup {
                                    let id = format!("fixture-warmup-{warmup_sequence}");
                                    warmup_sequence += 1;
                                    vec![responses::ev_response_created(&id), responses::ev_completed(&id)]
                                } else {
                                    let stream = client.post(&upstream)
                                        .json(&body)
                                        .send().await?
                                        .error_for_status()?
                                        .text().await?;
                                    stream.lines()
                                        .filter_map(|line| line.strip_prefix("data: "))
                                        .filter(|data| *data != "[DONE]")
                                        .map(serde_json::from_str)
                                        .collect::<std::result::Result<Vec<Value>, _>>()?
                                };
                                for event in events {
                                    if event["type"] == "response.output_item.done" {
                                        logical_history.push(event["item"].clone());
                                    }
                                    if event["type"] == "response.completed" {
                                        let id = event["response"]["id"].as_str().context("fixture response ID")?;
                                        response_history.insert(id.to_string(), logical_history.clone());
                                    }
                                    if socket.send(Message::Text(event.to_string().into())).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                            Ok(())
                        });
                    }
                    Some(result) = connections.join_next(), if !connections.is_empty() => {
                        result.expect("fixture connection task").expect("fixture model transport");
                    }
                }
            }
        });
        Ok(Self { uri, bridge })
    }

    pub(super) fn uri(&self) -> &str {
        &self.uri
    }
}

impl Drop for ResponsesWebSocketForwarder {
    fn drop(&mut self) {
        // Dropping the bridge's JoinSet also cancels its outstanding connections.
        self.bridge.abort();
    }
}
