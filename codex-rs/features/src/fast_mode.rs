use crate::Feature;
use crate::Features;

/// Capability required to resolve accelerated model routing.
pub struct AcceleratedRoutingGrant {
    _private: (),
}

/// Routing behavior established by the effective Fast mode policy.
pub enum FastModeRoutingPolicy {
    StandardRoutingRequired,
    AcceleratedRoutingPermitted(AcceleratedRoutingGrant),
}

impl Features {
    /// Classifies whether this feature set authorizes accelerated routing.
    pub fn fast_mode_routing_policy(&self) -> FastModeRoutingPolicy {
        if self.enabled(Feature::FastMode) {
            FastModeRoutingPolicy::AcceleratedRoutingPermitted(AcceleratedRoutingGrant {
                _private: (),
            })
        } else {
            FastModeRoutingPolicy::StandardRoutingRequired
        }
    }
}
