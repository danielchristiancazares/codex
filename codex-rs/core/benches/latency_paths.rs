use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_api::ApiError;
use codex_api::ResponseEvent;
use codex_api::WebsocketEventMetadata;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::ServerNotification;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::RolloutItem;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::InMemoryThreadStore;
use codex_thread_store::LiveThread;
use codex_thread_store::ThreadPersistenceMetadata;
use codex_utils_audio::estimate_audio_token_count;
use codex_utils_audio::prepare_response_items;
use divan::Bencher;
use divan::counter::BytesCount;
use divan::counter::ItemsCount;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio_tungstenite::tungstenite::Message;

#[path = "../src/utils/json.rs"]
#[allow(dead_code, unused_imports)]
mod json;

#[path = "latency_paths/responses_websocket.rs"]
mod responses_websocket;

const PCM_SAMPLE_COUNTS: [usize; 2] = [8_000, 5 * 1024 * 1024];
const TTFT_EVENT_COUNT: usize = 100_000;

fn main() {
    divan::main();
}

#[divan::bench(args = PCM_SAMPLE_COUNTS)]
fn pcm_wav_prepare(bencher: Bencher, sample_count: usize) {
    let item = ResponseItem::Message {
        id: Default::default(),
        role: "user".to_string(),
        content: vec![ContentItem::InputAudio {
            audio_url: pcm_wav_data_url(sample_count),
        }],
        phase: Default::default(),
        internal_chat_message_metadata_passthrough: Default::default(),
    };
    bencher
        .with_inputs(move || vec![item.clone()])
        .bench_local_values(|mut items| {
            prepare_response_items(&mut items);
            items
        });
}

#[divan::bench(args = PCM_SAMPLE_COUNTS)]
fn pcm_wav_estimate(bencher: Bencher, sample_count: usize) {
    let audio_url = pcm_wav_data_url(sample_count);
    #[allow(clippy::expect_used)]
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
    let _runtime_guard = runtime.enter();
    let _ = estimate_audio_token_count(&audio_url);
    bencher.bench_local(move || estimate_audio_token_count(&audio_url));
}

#[divan::bench]
fn one_mebibyte_json_counted_len(bencher: Bencher) {
    let value = one_mebibyte_json_value();
    bencher.bench_local(move || json::serialized_json_bytes(&value));
}

#[divan::bench(sample_count = 100, sample_size = 1000)]
fn websocket_telemetry_no_metrics_text_delta(bencher: Bencher) {
    assert!(codex_otel::global().is_none());
    let telemetry = SessionTelemetry::new(
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
    );
    let payload = serde_json::json!({
        "type": "response.output_text.delta",
        "sequence_number": 42,
        "delta": "abcdefghijklmnop",
    })
    .to_string();
    let payload_bytes = payload.len();
    let response: Result<_, ApiError> = Ok(Some(Ok(Message::Text(payload.into()))));

    bencher
        .counter(ItemsCount::new(/*count*/ 1usize))
        .counter(BytesCount::new(payload_bytes))
        .bench_local(move || {
            divan::black_box(&telemetry).record_websocket_event(
                divan::black_box(&response),
                Duration::from_micros(/*micros*/ 100),
            )
        });
}

#[divan::bench(sample_count = 100, sample_size = 100)]
fn websocket_telemetry_metrics_text_delta(bencher: Bencher) {
    #[allow(clippy::expect_used)]
    let metrics = MetricsClient::new(MetricsConfig::in_memory(
        "benchmark",
        "codex-core-benchmark",
        env!("CARGO_PKG_VERSION"),
        InMemoryMetricExporter::default(),
    ))
    .expect("benchmark metrics client should start");
    let telemetry = SessionTelemetry::new(
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
    )
    .with_metrics(metrics);
    let payload =
        r#"{"type":"response.output_text.delta","sequence_number":42,"delta":"abcdefghijklmnop"}"#;
    let payload_bytes = payload.len();
    let metadata = WebsocketEventMetadata {
        kind: "response.output_text.delta",
        payload,
    };
    telemetry.record_parsed_websocket_event(metadata, Duration::from_micros(/*micros*/ 100));

    bencher
        .counter(ItemsCount::new(/*count*/ 1usize))
        .counter(BytesCount::new(payload_bytes))
        .bench_local(move || {
            divan::black_box(&telemetry).record_parsed_websocket_event(
                divan::black_box(metadata),
                Duration::from_micros(/*micros*/ 100),
            )
        });
}

#[divan::bench(sample_count = 100, sample_size = 1000)]
fn response_telemetry_text_delta(bencher: Bencher) {
    let telemetry = SessionTelemetry::new(
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
    );
    let span = tracing::Span::none();
    assert!(span.is_disabled());
    let event = ResponseEvent::OutputTextDelta("abcdefghijklmnop".to_string());

    bencher
        .counter(ItemsCount::new(/*count*/ 1usize))
        .bench_local(move || {
            divan::black_box(&telemetry)
                .record_responses(divan::black_box(&span), divan::black_box(&event))
        });
}

#[divan::bench(sample_count = 100, sample_size = 1)]
fn turn_ttft_post_first_gate(bencher: Bencher) {
    #[allow(clippy::expect_used)]
    let runtime = Builder::new_current_thread()
        .build()
        .expect("benchmark runtime should start");
    let first_token_recorded = AtomicBool::new(true);

    bencher
        .counter(ItemsCount::new(TTFT_EVENT_COUNT))
        .bench_local(move || {
            runtime.block_on(async {
                for _ in 0..TTFT_EVENT_COUNT {
                    if divan::black_box(&first_token_recorded).load(Ordering::Acquire) {
                        divan::black_box(None::<Duration>);
                    }
                }
            })
        });
}

#[divan::bench(sample_count = 100, sample_size = 1000)]
fn server_notification_opt_out_lookup(bencher: Bencher) {
    let notification = ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
        thread_id: "thread-performance".to_string(),
        turn_id: "turn-performance".to_string(),
        item_id: "item-performance".to_string(),
        delta: "abcdefghijklmnop".to_string(),
    });
    let opted_out = HashSet::from(["item/reasoning/textDelta".to_string()]);

    bencher
        .counter(ItemsCount::new(/*count*/ 1usize))
        .bench_local(move || {
            divan::black_box(&opted_out).contains(divan::black_box(&notification).as_ref())
        });
}

#[divan::bench(sample_count = 100, sample_size = 1000)]
fn live_thread_transient_delta_append(bencher: Bencher) {
    assert!(codex_otel::global().is_none());
    #[allow(clippy::expect_used)]
    let runtime = Builder::new_current_thread()
        .build()
        .expect("benchmark runtime should start");
    let thread_id = ThreadId::new();
    let store = Arc::new(InMemoryThreadStore::default());
    #[allow(clippy::expect_used)]
    let live_thread = runtime
        .block_on(LiveThread::create(
            store,
            CreateThreadParams {
                session_id: thread_id.into(),
                thread_id,
                extra_config: None,
                forked_from_id: None,
                parent_thread_id: None,
                source: SessionSource::Exec,
                thread_source: None,
                originator: "benchmark".to_string(),
                base_instructions: BaseInstructions::default(),
                dynamic_tools: Vec::new(),
                selected_capability_roots: Vec::new(),
                multi_agent_version: None,
                history_mode: ThreadHistoryMode::Legacy,
                history_base: None,
                subagent_history_start_ordinal: None,
                initial_window_id: "window-performance".to_string(),
                metadata: ThreadPersistenceMetadata {
                    cwd: None,
                    model_provider: "benchmark-provider".to_string(),
                    memory_mode: ThreadMemoryMode::Enabled,
                },
            },
        ))
        .expect("benchmark live thread should start");
    let items = [RolloutItem::EventMsg(
        codex_protocol::protocol::EventMsg::AgentMessageContentDelta(
            AgentMessageContentDeltaEvent {
                thread_id: thread_id.to_string(),
                turn_id: "turn-performance".to_string(),
                item_id: "item-performance".to_string(),
                delta: "abcdefghijklmnop".to_string(),
            },
        ),
    )];
    assert!(!codex_rollout::is_persisted_rollout_item(
        &items[0],
        ThreadHistoryMode::Legacy
    ));

    bencher
        .counter(ItemsCount::new(/*count*/ 1usize))
        .bench_local(move || {
            divan::black_box(
                runtime.block_on(
                    divan::black_box(&live_thread).append_items(divan::black_box(&items)),
                ),
            )
        });
}

fn pcm_wav_data_url(sample_count: usize) -> String {
    let sample_count = u32::try_from(sample_count).unwrap_or(u32::MAX);
    let padding = sample_count % 2;
    let riff_size = 36u32.saturating_add(sample_count).saturating_add(padding);
    let mut bytes = Vec::with_capacity(riff_size as usize + 8);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8_000u32.to_le_bytes());
    bytes.extend_from_slice(&8_000u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&sample_count.to_le_bytes());
    bytes.resize(bytes.len() + sample_count as usize + padding as usize, 0);
    format!("data:audio/wav;base64,{}", BASE64_STANDARD.encode(bytes))
}

fn one_mebibyte_json_value() -> serde_json::Value {
    serde_json::json!({
        "call_id": "call-performance",
        "output": "x".repeat(1024 * 1024),
    })
}
