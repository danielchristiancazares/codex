use crate::context::ContextualUserFragment;
use crate::state::AdditionalContextSnapshot;

use super::PreviousSectionState;
use super::WorldStateSection;

pub(crate) struct AdditionalContextState {
    snapshot: AdditionalContextSnapshot,
}

impl AdditionalContextState {
    pub(crate) fn new(snapshot: AdditionalContextSnapshot) -> Self {
        Self { snapshot }
    }
}

impl WorldStateSection for AdditionalContextState {
    const ID: &'static str = "additional_context";
    type Snapshot = AdditionalContextSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        self.snapshot.clone()
    }

    fn render_diff(
        &self,
        _previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        None
    }
}

impl super::WorldStateSnapshot {
    /// Legacy rollouts start with an empty publication cache and remain readable.
    pub(crate) fn restore_additional_context(
        &self,
        store: &mut crate::state::AdditionalContextStore,
    ) {
        match self.sections.get(AdditionalContextState::ID) {
            Some(value) => match serde_json::from_value(value.clone()) {
                Ok(snapshot) => store.restore(snapshot),
                Err(error) => {
                    tracing::warn!(%error, "discarding malformed additional-context fingerprint snapshot");
                    store.restore(AdditionalContextSnapshot::default());
                }
            },
            None => store.restore(AdditionalContextSnapshot::default()),
        }
    }
}
