use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_api::ApiError;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_utils_audio::estimate_audio_token_count;
use codex_utils_audio::prepare_response_items;
use divan::Bencher;
use divan::counter::BytesCount;
use divan::counter::ItemsCount;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

#[path = "../src/utils/json.rs"]
#[allow(dead_code, unused_imports)]
mod json;

#[path = "latency_paths/responses_websocket.rs"]
mod responses_websocket;

const PCM_SAMPLE_COUNTS: [usize; 2] = [8_000, 5 * 1024 * 1024];

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
