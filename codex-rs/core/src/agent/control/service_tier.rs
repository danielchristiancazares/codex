//! Shares the root user's selected routing tier across the entire agent tree.

use super::AgentControl;
use codex_protocol::config_types::ServiceTier;
use std::sync::Arc;

impl AgentControl {
    /// Returns the latest user-selected tier for this root and all its descendants.
    pub(crate) fn root_service_tier(&self) -> ServiceTier {
        *self.root_service_tier.load_full()
    }

    /// Publishes a root-owned tier without mutating individual child sessions.
    pub(crate) fn set_root_service_tier(&self, service_tier: ServiceTier) {
        self.root_service_tier.store(Arc::new(service_tier));
    }
}
