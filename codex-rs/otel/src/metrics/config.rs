use crate::config::OtelExporter;
use crate::metrics::Result;
use crate::metrics::validation::validate_tag_key;
use crate::metrics::validation::validate_tag_value;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum MetricsExporter {
    Disabled,
}

#[derive(Clone, Debug)]
pub struct MetricsConfig {
    pub(crate) runtime_reader: bool,
    pub(crate) default_tags: BTreeMap<String, String>,
}

impl MetricsConfig {
    pub fn otlp(
        _environment: impl Into<String>,
        _service_name: impl Into<String>,
        _service_version: impl Into<String>,
        _exporter: OtelExporter,
    ) -> Self {
        Self {
            runtime_reader: false,
            default_tags: BTreeMap::new(),
        }
    }

    pub fn in_memory<T>(
        _environment: impl Into<String>,
        _service_name: impl Into<String>,
        _service_version: impl Into<String>,
        _exporter: T,
    ) -> Self {
        Self {
            runtime_reader: false,
            default_tags: BTreeMap::new(),
        }
    }

    pub fn with_export_interval(self, _interval: Duration) -> Self {
        self
    }

    pub fn with_runtime_reader(mut self) -> Self {
        self.runtime_reader = true;
        self
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Result<Self> {
        let key = key.into();
        let value = value.into();
        validate_tag_key(&key)?;
        validate_tag_value(&value)?;
        self.default_tags.insert(key, value);
        Ok(self)
    }
}
