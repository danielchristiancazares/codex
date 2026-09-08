//! Measures disabled production telemetry separately from event parsing and terminal rendering.

use codex_api::ApiError;
use codex_api::ResponseEvent;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use std::hint::black_box;
use std::time::Duration;
use std::time::Instant;
use tokio_tungstenite::tungstenite::Message;

fn measure(name: &str, mut operation: impl FnMut()) {
    let mut samples = Vec::new();
    for sample in 0..36 {
        let start = Instant::now();
        for _ in 0..100_000 {
            operation();
        }
        let ns = start.elapsed().as_nanos() as f64 / 100_000.0;
        if sample >= 5 {
            samples.push(ns);
        }
    }
    samples.sort_by(f64::total_cmp);
    println!("{name}_ns_per_event={:.3}", samples[samples.len() / 2]);
}

fn main() {
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
    let response: Result<_, ApiError> = Ok(Some(Ok(Message::Text(
        r#"{"type":"response.output_text.delta","sequence_number":42,"delta":"abcdefghijklmnop"}"#
            .into(),
    ))));
    measure("disabled_websocket_telemetry", || {
        black_box(&telemetry)
            .record_websocket_event(black_box(&response), Duration::from_micros(/*micros*/ 100));
    });
    let span = tracing::Span::none();
    assert!(span.is_disabled());
    let event = ResponseEvent::OutputTextDelta("abcdefghijklmnop".to_string());
    measure("disabled_response_span", || {
        black_box(&telemetry).record_responses(black_box(&span), black_box(&event));
    });
}
