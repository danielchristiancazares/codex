use crate::config::OtelSettings;
use crate::metrics::MetricsClient;
use crate::targets::is_log_export_target;
use crate::targets::is_trace_safe_target;
use std::error::Error;
use std::io;
use std::time::Duration;
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;

#[derive(Debug, Default)]
pub struct OtelProvider {
    pub logger: Option<()>,
    pub tracer_provider: Option<()>,
    pub tracer: Option<()>,
    pub metrics: Option<MetricsClient>,
}

impl OtelProvider {
    pub fn shutdown(&self) {}

    pub async fn shutdown_with_timeout(self, _timeout: Duration) -> io::Result<()> {
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
        self.metrics.as_ref()
    }
}
