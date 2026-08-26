use super::resolve_request_service_tier;
use crate::config_types::ServiceTier;
use crate::openai_models::ModelServiceTier;
use pretty_assertions::assert_eq;

fn advertised_service_tier(id: ServiceTier) -> ModelServiceTier {
    ModelServiceTier {
        id,
        name: id.to_string(),
        description: format!("{id} routing"),
    }
}

#[test]
fn standard_routing_remains_standard_with_accelerated_tiers_advertised() {
    let advertised_service_tiers = [
        advertised_service_tier(ServiceTier::Fast),
        advertised_service_tier(ServiceTier::Flex),
    ];

    assert_eq!(
        resolve_request_service_tier(&advertised_service_tiers, ServiceTier::Default),
        ServiceTier::Default
    );
}

#[test]
fn advertised_accelerated_routing_is_retained() {
    let advertised_service_tiers = [
        advertised_service_tier(ServiceTier::Fast),
        advertised_service_tier(ServiceTier::Flex),
    ];

    assert_eq!(
        [
            resolve_request_service_tier(&advertised_service_tiers, ServiceTier::Fast),
            resolve_request_service_tier(&advertised_service_tiers, ServiceTier::Flex),
        ],
        [ServiceTier::Fast, ServiceTier::Flex]
    );
}

#[test]
fn unadvertised_accelerated_routing_uses_standard_routing() {
    let advertised_service_tiers = [advertised_service_tier(ServiceTier::Flex)];

    assert_eq!(
        resolve_request_service_tier(&advertised_service_tiers, ServiceTier::Fast),
        ServiceTier::Default
    );
}

#[test]
fn legacy_fast_deserializes_to_canonical_priority_routing() {
    let service_tier: ServiceTier =
        serde_json::from_str("\"fast\"").expect("legacy service tier should deserialize");

    assert_eq!(service_tier, ServiceTier::Fast);
    assert_eq!(
        serde_json::to_string(&service_tier).expect("service tier should serialize"),
        "\"priority\""
    );
}
