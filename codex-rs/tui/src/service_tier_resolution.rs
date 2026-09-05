//! Resolves the concrete service tier used by TUI turns.
//!
//! The TUI keeps service-tier state as [`ServiceTier`] throughout. A configured
//! non-default tier is honored only when the active model supports it. The
//! catalog default applies when Fast mode is enabled unless the persisted
//! notice records an explicit opt-out.

use crate::legacy_core::config::Config;
use codex_features::Feature;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelPreset;

pub(crate) fn effective_service_tier(
    config: &Config,
    notices: &codex_config::types::Notice,
    model: &str,
    models: &[ModelPreset],
) -> ServiceTier {
    if !config.features.enabled(Feature::FastMode) {
        return ServiceTier::Default;
    }

    let configured = config.service_tier;
    let Some(preset) = models.iter().find(|preset| preset.model == model) else {
        return configured;
    };

    if configured.is_default() {
        if notices.fast_default_opt_out == Some(true) {
            return ServiceTier::Default;
        }
        let catalog_default = preset.default_service_tier;
        return model_supports_service_tier(preset, catalog_default)
            .then_some(catalog_default)
            .unwrap_or_default();
    }

    model_supports_service_tier(preset, configured)
        .then_some(configured)
        .unwrap_or_default()
}

fn model_supports_service_tier(model: &ModelPreset, service_tier: ServiceTier) -> bool {
    match service_tier {
        ServiceTier::Fast => model.supports_fast_mode(),
        ServiceTier::Flex => model.supports_service_tier(ServiceTier::Flex),
        ServiceTier::Default => true,
    }
}
