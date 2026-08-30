use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::ModelSwitchInstructions;
use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum ModelInstructionsSnapshot {
    Current {
        model: String,
        instructions_sha1: String,
    },
    Legacy(String),
}

/// Model identity and the instructions needed when that identity changes.
#[derive(Clone, Debug)]
pub(crate) struct ModelInstructionsState {
    model: String,
    previous_model: Option<String>,
    instructions: String,
    instructions_sha1: String,
}

impl ModelInstructionsState {
    pub(crate) fn new(model: &str, previous_model: Option<&str>, instructions: String) -> Self {
        let instructions_sha1 = format!("{:x}", Sha1::digest(instructions.as_bytes()));
        Self {
            model: model.to_string(),
            previous_model: previous_model.map(str::to_string),
            instructions,
            instructions_sha1,
        }
    }
}

impl WorldStateSection for ModelInstructionsState {
    const ID: &'static str = "model";
    type Snapshot = ModelInstructionsSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        ModelInstructionsSnapshot::Current {
            model: self.model.clone(),
            instructions_sha1: self.instructions_sha1.clone(),
        }
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && ModelSwitchInstructions::matches_text(text)
    }

    fn has_retained_fragment_matcher() -> bool {
        true
    }

    fn matches_retained_fragment(role: &str, text: &str) -> bool {
        Self::matches_legacy_fragment(role, text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        let model_changed = match previous {
            PreviousSectionState::Known(ModelInstructionsSnapshot::Current {
                model,
                instructions_sha1,
            }) => model != &self.model && instructions_sha1 != &self.instructions_sha1,
            PreviousSectionState::Known(ModelInstructionsSnapshot::Legacy(previous_model)) => {
                previous_model != &self.model
            }
            PreviousSectionState::Unknown | PreviousSectionState::Absent => self
                .previous_model
                .as_deref()
                .is_some_and(|previous| previous != self.model),
        };

        (model_changed && !self.instructions.is_empty()).then(|| {
            Box::new(ModelSwitchInstructions::new(self.instructions.clone()))
                as Box<dyn ContextualUserFragment>
        })
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
