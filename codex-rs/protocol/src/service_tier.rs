use crate::config_types::ServiceTier;
use crate::openai_models::ModelServiceTier;

/// Resolves a requested service tier against the finite advertised set.
///
/// Standard routing remains standard. Concrete accelerated tiers remain selected
/// only when the same domain value appears in the catalog.
pub fn resolve_request_service_tier(
    advertised_service_tiers: &[ModelServiceTier],
    requested_service_tier: ServiceTier,
) -> ServiceTier {
    match advertised_service_tiers {
        [] => ServiceTier::Default,
        [advertised_service_tier, remaining_service_tiers @ ..] => {
            match (requested_service_tier, advertised_service_tier.id) {
                (
                    ServiceTier::Default,
                    ServiceTier::Default | ServiceTier::Fast | ServiceTier::Flex,
                ) => ServiceTier::Default,
                (ServiceTier::Fast, ServiceTier::Fast) => ServiceTier::Fast,
                (ServiceTier::Flex, ServiceTier::Flex) => ServiceTier::Flex,
                (ServiceTier::Fast, ServiceTier::Default | ServiceTier::Flex)
                | (ServiceTier::Flex, ServiceTier::Default | ServiceTier::Fast) => {
                    resolve_request_service_tier(remaining_service_tiers, requested_service_tier)
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "service_tier_tests.rs"]
mod tests;
