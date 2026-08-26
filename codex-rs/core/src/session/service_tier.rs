use super::Session;
use codex_features::AcceleratedRoutingGrant;
use codex_features::FastModeRoutingPolicy;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelServiceTier;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;

/// Resolution of configured service-tier routing at session startup.
///
/// Each variant records either the routing behavior selected for the session
/// or the exact fact that forced standard routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InitialServiceTierResolution {
    ConfiguredStandardRouting,
    AdvertisedFastRouting,
    AdvertisedFlexRouting,
    DisabledFastUsesStandardRouting,
    DisabledFlexUsesStandardRouting,
    UnadvertisedFastUsesStandardRouting,
    UnadvertisedFlexUsesStandardRouting,
}

pub(super) fn resolve_initial_service_tier(
    configured_service_tier: ServiceTier,
    routing_policy: FastModeRoutingPolicy,
    advertised_service_tiers: &[ModelServiceTier],
) -> InitialServiceTierResolution {
    match configured_service_tier {
        ServiceTier::Default => InitialServiceTierResolution::ConfiguredStandardRouting,
        ServiceTier::Fast => match routing_policy {
            FastModeRoutingPolicy::StandardRoutingRequired => {
                InitialServiceTierResolution::DisabledFastUsesStandardRouting
            }
            FastModeRoutingPolicy::AcceleratedRoutingPermitted(grant) => {
                resolve_advertised_fast_routing(grant, advertised_service_tiers)
            }
        },
        ServiceTier::Flex => match routing_policy {
            FastModeRoutingPolicy::StandardRoutingRequired => {
                InitialServiceTierResolution::DisabledFlexUsesStandardRouting
            }
            FastModeRoutingPolicy::AcceleratedRoutingPermitted(grant) => {
                resolve_advertised_flex_routing(grant, advertised_service_tiers)
            }
        },
    }
}

fn resolve_advertised_fast_routing(
    grant: AcceleratedRoutingGrant,
    advertised_service_tiers: &[ModelServiceTier],
) -> InitialServiceTierResolution {
    match advertised_service_tiers {
        [] => InitialServiceTierResolution::UnadvertisedFastUsesStandardRouting,
        [advertised_service_tier, remaining_service_tiers @ ..] => {
            match advertised_service_tier.id {
                ServiceTier::Fast => InitialServiceTierResolution::AdvertisedFastRouting,
                ServiceTier::Default | ServiceTier::Flex => {
                    resolve_advertised_fast_routing(grant, remaining_service_tiers)
                }
            }
        }
    }
}

fn resolve_advertised_flex_routing(
    grant: AcceleratedRoutingGrant,
    advertised_service_tiers: &[ModelServiceTier],
) -> InitialServiceTierResolution {
    match advertised_service_tiers {
        [] => InitialServiceTierResolution::UnadvertisedFlexUsesStandardRouting,
        [advertised_service_tier, remaining_service_tiers @ ..] => {
            match advertised_service_tier.id {
                ServiceTier::Flex => InitialServiceTierResolution::AdvertisedFlexRouting,
                ServiceTier::Default | ServiceTier::Fast => {
                    resolve_advertised_flex_routing(grant, remaining_service_tiers)
                }
            }
        }
    }
}

impl InitialServiceTierResolution {
    pub(super) const fn session_service_tier(self) -> ServiceTier {
        match self {
            Self::ConfiguredStandardRouting
            | Self::DisabledFastUsesStandardRouting
            | Self::DisabledFlexUsesStandardRouting => ServiceTier::Default,
            Self::AdvertisedFastRouting | Self::UnadvertisedFastUsesStandardRouting => {
                ServiceTier::Fast
            }
            Self::AdvertisedFlexRouting | Self::UnadvertisedFlexUsesStandardRouting => {
                ServiceTier::Flex
            }
        }
    }

    pub(super) async fn emit_startup_warning(self, session: &Session) {
        match self {
            Self::UnadvertisedFastUsesStandardRouting => {
                session
                    .send_event_raw(Event {
                        id: super::INITIAL_SUBMIT_ID.to_owned(),
                        msg: EventMsg::Warning(WarningEvent {
                            message: "Configured service tier `fast` is not advertised by the resolved model and will use standard routing.".to_string(),
                        }),
                    })
                    .await;
            }
            Self::UnadvertisedFlexUsesStandardRouting => {
                session
                    .send_event_raw(Event {
                        id: super::INITIAL_SUBMIT_ID.to_owned(),
                        msg: EventMsg::Warning(WarningEvent {
                            message: "Configured service tier `flex` is not advertised by the resolved model and will use standard routing.".to_string(),
                        }),
                    })
                    .await;
            }
            Self::ConfiguredStandardRouting
            | Self::AdvertisedFastRouting
            | Self::AdvertisedFlexRouting
            | Self::DisabledFastUsesStandardRouting
            | Self::DisabledFlexUsesStandardRouting => {}
        }
    }
}

#[cfg(test)]
#[path = "service_tier_tests.rs"]
mod tests;
