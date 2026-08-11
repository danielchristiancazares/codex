use super::*;
use codex_protocol::protocol::W3cTraceContext;
use pretty_assertions::assert_eq;

#[test]
fn provider_is_always_disabled() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let settings = OtelSettings {
        environment: "test".to_string(),
        service_name: "codex-test".to_string(),
        service_version: "0".to_string(),
        codex_home: std::path::PathBuf::new(),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::None,
        metrics_exporter: OtelExporter::None,
        runtime_metrics: true,
        span_attributes: Default::default(),
        tracestate: Default::default(),
    };

    assert!(OtelProvider::from(&settings)?.is_none());
    Ok(())
}

#[test]
fn metrics_client_accepts_calls_without_recording_runtime_data() -> Result<()> {
    let metrics = MetricsClient::new(
        MetricsConfig::otlp("test", "codex-test", "0", OtelExporter::None).with_runtime_reader(),
    )?;

    metrics.counter("codex.test.count", 1, &[("kind", "noop")])?;
    metrics.histogram("codex.test.duration_ms", 12, &[])?;
    assert_eq!(metrics.snapshot::<ResourceMetrics>()?, ResourceMetrics);
    Ok(())
}

#[test]
fn trace_context_parser_keeps_validation_without_exporting_spans() {
    let valid = W3cTraceContext {
        traceparent: Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string()),
        tracestate: None,
    };
    let invalid = W3cTraceContext {
        traceparent: Some("invalid".to_string()),
        tracestate: None,
    };

    assert!(context_from_w3c_trace_context(&valid).is_some());
    assert!(context_from_w3c_trace_context(&invalid).is_none());
    assert_eq!(current_span_w3c_trace_context(), None);
}
