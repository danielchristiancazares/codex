//! Refreshes only future-turn capacity from the rebuilt thread-owned layer stack.

use super::session::SessionConfiguration;
use crate::config::Config;
use codex_protocol::openai_models::ContextWindowCapacity;

enum CapacityRefresh {
    UseCatalog,
    Override(ContextWindowCapacity),
}

#[derive(Debug, thiserror::Error)]
#[error("model_context_window must be a positive integer")]
pub(super) struct InvalidCapacityRefresh;

impl CapacityRefresh {
    fn from_config(config: &Config) -> Result<Self, InvalidCapacityRefresh> {
        match config
            .config_layer_stack
            .effective_config()
            .get("model_context_window")
        {
            None => Ok(Self::UseCatalog),
            Some(value) => Ok(Self::Override(
                ContextWindowCapacity::try_from(value.as_integer().ok_or(InvalidCapacityRefresh)?)
                    .map_err(|_| InvalidCapacityRefresh)?,
            )),
        }
    }

    fn apply(self, session: &mut SessionConfiguration, config: &mut Config) {
        match self {
            Self::UseCatalog => {
                config.model_context_window = None;
                session.model_info_overrides.context_window = None;
            }
            Self::Override(capacity) => {
                config.model_context_window = Some(capacity.tokens());
                session.model_info_overrides.context_window = Some(capacity.tokens());
            }
        }
    }
}

impl SessionConfiguration {
    pub(super) fn refresh_model_capacity(
        &mut self,
        config: &mut Config,
    ) -> Result<(), InvalidCapacityRefresh> {
        CapacityRefresh::from_config(config)?.apply(self, config);
        Ok(())
    }
}
