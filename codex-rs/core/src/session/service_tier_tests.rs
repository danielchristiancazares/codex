use super::InitialServiceTierResolution;
use super::resolve_initial_service_tier;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelServiceTier;
use pretty_assertions::assert_eq;

fn advertised_fast_routing() -> Vec<ModelServiceTier> {
    vec![ModelServiceTier {
        id: ServiceTier::Fast,
        name: "Fast".to_string(),
        description: "Priority processing.".to_string(),
    }]
}

#[test]
fn startup_resolution_names_every_routing_consequence() {
    let advertised_service_tiers = advertised_fast_routing();
    let mut enabled_features = Features::default();
    enabled_features.enable(Feature::FastMode);
    let mut disabled_features = enabled_features.clone();
    disabled_features.disable(Feature::FastMode);

    assert_eq!(
        [
            resolve_initial_service_tier(
                ServiceTier::Default,
                enabled_features.fast_mode_routing_policy(),
                &advertised_service_tiers,
            ),
            resolve_initial_service_tier(
                ServiceTier::Fast,
                enabled_features.fast_mode_routing_policy(),
                &advertised_service_tiers,
            ),
            resolve_initial_service_tier(
                ServiceTier::Flex,
                enabled_features.fast_mode_routing_policy(),
                &advertised_service_tiers,
            ),
            resolve_initial_service_tier(
                ServiceTier::Fast,
                disabled_features.fast_mode_routing_policy(),
                &advertised_service_tiers,
            ),
            resolve_initial_service_tier(
                ServiceTier::Flex,
                disabled_features.fast_mode_routing_policy(),
                &advertised_service_tiers,
            ),
        ],
        [
            InitialServiceTierResolution::ConfiguredStandardRouting,
            InitialServiceTierResolution::AdvertisedFastRouting,
            InitialServiceTierResolution::UnadvertisedFlexUsesStandardRouting,
            InitialServiceTierResolution::DisabledFastUsesStandardRouting,
            InitialServiceTierResolution::DisabledFlexUsesStandardRouting,
        ]
    );
}

#[test]
fn startup_resolution_retains_authorized_selection_for_later_models() {
    assert_eq!(
        [
            InitialServiceTierResolution::ConfiguredStandardRouting.session_service_tier(),
            InitialServiceTierResolution::AdvertisedFastRouting.session_service_tier(),
            InitialServiceTierResolution::AdvertisedFlexRouting.session_service_tier(),
            InitialServiceTierResolution::UnadvertisedFastUsesStandardRouting
                .session_service_tier(),
            InitialServiceTierResolution::UnadvertisedFlexUsesStandardRouting
                .session_service_tier(),
            InitialServiceTierResolution::DisabledFastUsesStandardRouting.session_service_tier(),
            InitialServiceTierResolution::DisabledFlexUsesStandardRouting.session_service_tier(),
        ],
        [
            ServiceTier::Default,
            ServiceTier::Fast,
            ServiceTier::Flex,
            ServiceTier::Fast,
            ServiceTier::Flex,
            ServiceTier::Default,
            ServiceTier::Default,
        ]
    );
}
