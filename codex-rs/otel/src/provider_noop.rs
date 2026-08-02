use crate::config::OtelSettings;
use crate::metrics::MetricsClient;
use crate::targets::is_log_export_target;
use crate::targets::is_trace_safe_target;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::Tracer;
use std::error::Error;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

pub struct OtelProvider {
    pub logger: Option<SdkLoggerProvider>,
    pub tracer_provider: Option<SdkTracerProvider>,
    pub tracer: Option<Tracer>,
    pub metrics: Option<MetricsClient>,
    shutdown_started: AtomicBool,
}

impl OtelProvider {
    pub fn shutdown(&self) {
        self.shutdown_started.store(true, Ordering::Release);
    }

    pub async fn shutdown_with_timeout(self, _timeout: Duration) -> io::Result<()> {
        self.shutdown();
        Ok(())
    }

    pub fn from(_settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        Ok(None)
    }

    pub fn logger_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        None::<tracing_subscriber::layer::Identity>
    }

    pub fn logger_export_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        None::<tracing_subscriber::layer::Identity>
    }

    pub fn tracing_layer<S>(&self) -> Option<impl Layer<S> + Send + Sync>
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        None::<tracing_subscriber::layer::Identity>
    }

    pub fn reloadable_tracing_layer<S>(_service_name: &'static str) -> impl Layer<S> + Send + Sync
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
    {
        tracing_subscriber::layer::Identity::new()
    }

    pub fn codex_export_filter(meta: &tracing::Metadata<'_>) -> bool {
        Self::log_export_filter(meta)
    }

    pub fn log_export_filter(meta: &tracing::Metadata<'_>) -> bool {
        is_log_export_target(meta.target())
    }

    pub fn trace_export_filter(meta: &tracing::Metadata<'_>) -> bool {
        meta.is_span() || is_trace_safe_target(meta.target())
    }

    pub fn metrics(&self) -> Option<&MetricsClient> {
        None
    }
}

impl Drop for OtelProvider {
    fn drop(&mut self) {
        self.shutdown();
    }
}
