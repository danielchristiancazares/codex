use codex_api::ApiError;
use codex_api::Provider;
use codex_api::ResponseCreateWsRequest;
use codex_api::ResponseEvent;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesWebsocketClient;
use codex_api::ResponsesWebsocketConnection;
use codex_api::ResponsesWsRequest;
use codex_api::RetryConfig;
use codex_api::WebsocketTelemetry;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_model_provider::unauthenticated_auth_provider;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::protocol::SessionSource;
use divan::Bencher;
use divan::counter::BytesCount;
use divan::counter::ItemsCount;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio_tungstenite::accept_hdr_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::extensions::ExtensionsConfig;
use tokio_tungstenite::tungstenite::extensions::compression::deflate::DeflateConfig;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::proxy::ProxyConfig;

const DELTA_EVENT_COUNT: usize = 16_384;
const RESPONSE_EVENT_COUNT: usize = DELTA_EVENT_COUNT + 1;
const DELTA_TEXT: &str = "abcdefghijklmnop";
const RESPONSE_ID: &str = "resp-benchmark";
const DELTA_PAYLOAD: &str =
    r#"{"type":"response.output_text.delta","sequence_number":42,"delta":"abcdefghijklmnop"}"#;
const COMPLETED_PAYLOAD: &str = r#"{"type":"response.completed","response":{"id":"resp-benchmark","usage":{"input_tokens":0,"input_tokens_details":null,"output_tokens":0,"output_tokens_details":null,"total_tokens":0}}}"#;

#[derive(Debug, PartialEq, Eq)]
struct ResponseSummary {
    delta_events: usize,
    delta_bytes: usize,
    completion_seen: bool,
    unexpected_events: usize,
}

const EXPECTED_SUMMARY: ResponseSummary = ResponseSummary {
    delta_events: DELTA_EVENT_COUNT,
    delta_bytes: DELTA_EVENT_COUNT * DELTA_TEXT.len(),
    completion_seen: true,
    unexpected_events: 0,
};

struct BenchmarkWebsocketTelemetry {
    session: SessionTelemetry,
}

impl WebsocketTelemetry for BenchmarkWebsocketTelemetry {
    fn on_ws_request(
        &self,
        _duration: Duration,
        _error: Option<&ApiError>,
        _connection_reused: bool,
    ) {
    }

    fn on_ws_event(
        &self,
        result: &Result<Option<Result<Message, tokio_tungstenite::tungstenite::Error>>, ApiError>,
        duration: Duration,
    ) {
        self.session.record_websocket_event(result, duration);
    }
}

struct ResponsesWebsocketBenchmark {
    connection: ResponsesWebsocketConnection,
    request: ResponsesApiRequest,
    response_payload_bytes: usize,
    runtime: Runtime,
    _server_runtime: Runtime,
}

impl ResponsesWebsocketBenchmark {
    fn new() -> Self {
        assert!(codex_otel::global().is_none());

        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => panic!("failed to create benchmark runtime: {error}"),
        };
        let server_runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(/*val*/ 1)
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => panic!("failed to create benchmark server runtime: {error}"),
        };
        let (listener, base_url) = server_runtime.block_on(async {
            let listener = match TcpListener::bind("127.0.0.1:0").await {
                Ok(listener) => listener,
                Err(error) => panic!("failed to bind benchmark websocket server: {error}"),
            };
            let address = match listener.local_addr() {
                Ok(address) => address,
                Err(error) => panic!("failed to read benchmark websocket address: {error}"),
            };
            (listener, format!("ws://{address}"))
        });

        let delta = Message::Text(DELTA_PAYLOAD.into());
        let mut response_frames = vec![delta; DELTA_EVENT_COUNT];
        response_frames.push(Message::Text(COMPLETED_PAYLOAD.into()));
        let compression_negotiated = Arc::new(AtomicBool::new(false));
        server_runtime.spawn(serve_responses(
            listener,
            Arc::new(response_frames),
            Arc::clone(&compression_negotiated),
        ));

        let websocket_uri = match base_url.parse() {
            Ok(uri) => uri,
            Err(error) => panic!("failed to parse benchmark websocket URI: {error}"),
        };
        let proxy = match ProxyConfig::from_env(&websocket_uri) {
            Ok(proxy) => proxy,
            Err(error) => panic!("failed to resolve benchmark websocket proxy: {error}"),
        };
        assert!(
            proxy.is_none(),
            "benchmark requires a direct loopback route; configure NO_PROXY for 127.0.0.1"
        );

        let provider = Provider {
            name: "benchmark".to_string(),
            base_url,
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::ZERO,
                retry_429: false,
                retry_5xx: false,
                retry_transport: false,
            },
            stream_idle_timeout: Duration::from_secs(/*secs*/ 5),
        };
        let websocket_client =
            ResponsesWebsocketClient::new(provider, unauthenticated_auth_provider());
        let http_client_factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
        let telemetry = Arc::new(BenchmarkWebsocketTelemetry {
            session: SessionTelemetry::new(
                ThreadId::new(),
                "benchmark-model",
                "benchmark-model",
                /*account_id*/ None,
                /*account_email*/ None,
                /*auth_mode*/ None,
                "benchmark".to_string(),
                /*log_user_prompts*/ false,
                "benchmark".to_string(),
                SessionSource::Cli,
            ),
        });
        let connection = match runtime.block_on(websocket_client.connect(
            &http_client_factory,
            HeaderMap::new(),
            HeaderMap::new(),
            /*turn_state*/ None,
            Some(telemetry),
        )) {
            Ok(connection) => connection,
            Err(error) => panic!("failed to connect benchmark websocket: {error}"),
        };
        assert!(
            compression_negotiated.load(Ordering::Acquire),
            "benchmark websocket did not negotiate permessage-deflate"
        );

        Self {
            connection,
            request: ResponsesApiRequest {
                model: "benchmark-model".to_string(),
                instructions: String::new(),
                input: Vec::new(),
                tools: None,
                tool_choice: "auto".to_string(),
                parallel_tool_calls: false,
                reasoning: None,
                store: false,
                stream: true,
                stream_options: None,
                include: Vec::new(),
                service_tier: ServiceTier::Default,
                prompt_cache_key: None,
                text: None,
                client_metadata: None,
                access_programs: None,
            },
            response_payload_bytes: DELTA_PAYLOAD.len() * DELTA_EVENT_COUNT
                + COMPLETED_PAYLOAD.len(),
            runtime,
            _server_runtime: server_runtime,
        }
    }

    fn run_once(&self) -> ResponseSummary {
        self.runtime.block_on(async {
            let request =
                ResponsesWsRequest::ResponseCreate(ResponseCreateWsRequest::from(&self.request));
            let mut stream = match self
                .connection
                .stream_request(
                    request, /*connection_reused*/ true, /*turn_state*/ None,
                )
                .await
            {
                Ok(stream) => stream,
                Err(error) => panic!("failed to start benchmark websocket request: {error}"),
            };
            let mut summary = ResponseSummary {
                delta_events: 0,
                delta_bytes: 0,
                completion_seen: false,
                unexpected_events: 0,
            };
            let mut completion_received = false;

            while let Some(event) = stream.next().await {
                match event {
                    Ok(ResponseEvent::OutputTextDelta(delta)) => {
                        summary.delta_events += 1;
                        summary.delta_bytes += delta.len();
                    }
                    Ok(ResponseEvent::Completed { response_id, .. }) => {
                        if completion_received {
                            summary.unexpected_events += 1;
                            continue;
                        }
                        completion_received = true;
                        summary.completion_seen = response_id == RESPONSE_ID;
                    }
                    Ok(
                        ResponseEvent::Created
                        | ResponseEvent::SafetyBuffering(_)
                        | ResponseEvent::OutputItemDone(_)
                        | ResponseEvent::OutputItemAdded(_)
                        | ResponseEvent::ServerModel(_)
                        | ResponseEvent::ModelVerifications(_)
                        | ResponseEvent::TurnModerationMetadata(_)
                        | ResponseEvent::ServerReasoningIncluded(_)
                        | ResponseEvent::ToolCallInputDelta { .. }
                        | ResponseEvent::ReasoningSummaryDelta { .. }
                        | ResponseEvent::ReasoningSummaryDone { .. }
                        | ResponseEvent::ReasoningContentDelta { .. }
                        | ResponseEvent::ReasoningSummaryPartAdded { .. }
                        | ResponseEvent::RateLimits(_)
                        | ResponseEvent::ModelsEtag(_),
                    ) => summary.unexpected_events += 1,
                    Err(error) => panic!("benchmark websocket stream failed: {error}"),
                }
            }

            summary
        })
    }
}

async fn serve_responses(
    listener: TcpListener,
    response_frames: Arc<Vec<Message>>,
    compression_negotiated: Arc<AtomicBool>,
) {
    let Ok((stream, _address)) = listener.accept().await else {
        return;
    };
    let callback = move |_request: &Request, response: Response| {
        let negotiated = response
            .headers()
            .get(http::header::SEC_WEBSOCKET_EXTENSIONS)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("permessage-deflate"));
        compression_negotiated.store(negotiated, Ordering::Release);
        Ok(response)
    };
    let Ok(mut websocket) =
        accept_hdr_async_with_config(stream, callback, Some(websocket_server_config())).await
    else {
        return;
    };

    while let Some(Ok(message)) = websocket.next().await {
        if !matches!(message, Message::Text(_) | Message::Binary(_)) {
            continue;
        }
        for frame in response_frames.iter() {
            if websocket.feed(frame.clone()).await.is_err() {
                return;
            }
        }
        if websocket.flush().await.is_err() {
            return;
        }
    }
}

fn websocket_server_config() -> WebSocketConfig {
    let mut extensions = ExtensionsConfig::default();
    extensions.permessage_deflate = Some(DeflateConfig::default());

    let mut config = WebSocketConfig::default();
    config.extensions = extensions;
    config
}

#[divan::bench(sample_count = 50, sample_size = 1)]
fn responses_websocket_16384_text_deltas(bencher: Bencher) {
    let harness = ResponsesWebsocketBenchmark::new();
    assert_eq!(harness.run_once(), EXPECTED_SUMMARY);

    bencher
        .counter(ItemsCount::new(RESPONSE_EVENT_COUNT))
        .counter(BytesCount::new(harness.response_payload_bytes))
        .bench_local(|| {
            let summary = harness.run_once();
            assert_eq!(summary, EXPECTED_SUMMARY);
            summary
        });
}
