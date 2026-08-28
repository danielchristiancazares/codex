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
