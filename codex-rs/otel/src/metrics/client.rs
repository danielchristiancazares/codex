use crate::metrics::MetricsConfig;
use crate::metrics::MetricsError;
use crate::metrics::Result;
use crate::metrics::Timer;
use crate::metrics::validation::validate_metric_name;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use crate::metrics::validation::validate_tags;
use std::time::Duration;

/// Empty metrics snapshot returned by the disabled telemetry backend.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceMetrics;

/// API-compatible no-op metrics client.
#[derive(Clone, Debug)]
pub struct MetricsClient {
    runtime_reader: bool,
}

impl MetricsClient {
    pub fn new(config: MetricsConfig) -> Result<Self> {
        validate_tags(&config.default_tags)?;
        Ok(Self {
            runtime_reader: config.runtime_reader,
        })
    }

    pub fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        if inc < 0 {
            return Err(MetricsError::NegativeCounterIncrement {
                name: name.to_string(),
                inc,
            });
        }
        validate_tag_pairs(tags)
    }

    pub fn counter_with_description(
        &self,
        name: &str,
        _description: &str,
        inc: i64,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.counter(name, inc, tags)
    }

    pub fn histogram(&self, name: &str, _value: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        validate_tag_pairs(tags)
    }

    pub fn gauge(&self, name: &str, _value: i64, tags: &[(&str, &str)]) -> Result<()> {
        validate_metric_name(name)?;
        validate_tag_pairs(tags)
    }

    pub fn gauge_with_description(
        &self,
        name: &str,
        _description: &str,
        value: i64,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.gauge(name, value, tags)
    }

    pub fn register_observable_gauge_with_description(
        &self,
        name: &str,
        _description: &str,
        _observe: impl Fn() -> i64 + Send + Sync + 'static,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        validate_metric_name(name)?;
        validate_tag_pairs(tags)
    }

    pub fn record_duration(
        &self,
        name: &str,
        _duration: Duration,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        validate_metric_name(name)?;
        validate_tag_pairs(tags)
    }

    pub(crate) fn record_duration_ms_f64(
        &self,
        name: &str,
        _duration_ms: f64,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        validate_metric_name(name)?;
        validate_tag_pairs(tags)
    }

    pub fn record_duration_seconds_with_description(
        &self,
        name: &str,
        _description: &str,
        duration: Duration,
        tags: &[(&str, &str)],
    ) -> Result<()> {
        self.record_duration(name, duration, tags)
    }

    pub fn start_timer(&self, name: &str, tags: &[(&str, &str)]) -> Result<Timer> {
        validate_metric_name(name)?;
        validate_tag_pairs(tags)?;
        Ok(Timer::new(name, tags, self))
    }

    pub fn snapshot<T: Default>(&self) -> Result<T> {
        if !self.runtime_reader {
            return Err(MetricsError::RuntimeSnapshotUnavailable);
        }
        Ok(T::default())
    }

    pub fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

fn validate_tag_pairs(tags: &[(&str, &str)]) -> Result<()> {
    for (key, value) in tags {
        validate_tag_key(key)?;
        validate_tag_value(value)?;
    }
    Ok(())
}
