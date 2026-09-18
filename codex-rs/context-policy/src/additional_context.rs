use std::collections::BTreeMap;

use super::context_fingerprint::ContextFingerprint;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_protocol::turn_input::AdditionalContextAction;
use serde::Deserialize;
use serde::Serialize;

use codex_context_fragments::AdditionalContextDeveloperFragment;
use codex_context_fragments::AdditionalContextUserFragment;
use codex_context_fragments::ContextualUserFragment;

/// Serializable fingerprints of the publications represented by retained history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionalContextSnapshot {
    entries: BTreeMap<String, AdditionalContextSnapshotEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AdditionalContextSnapshotEntry {
    treatment: AdditionalContextTreatment,
    value_fingerprint: ContextFingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AdditionalContextTreatment {
    PresentAsUntrustedInput,
    PresentAsApplicationInstruction,
}

impl AdditionalContextTreatment {
    fn from_kind(kind: AdditionalContextKind) -> Self {
        match kind {
            AdditionalContextKind::Untrusted => Self::PresentAsUntrustedInput,
            AdditionalContextKind::Application => Self::PresentAsApplicationInstruction,
        }
    }

    fn kind(self) -> AdditionalContextKind {
        match self {
            Self::PresentAsUntrustedInput => AdditionalContextKind::Untrusted,
            Self::PresentAsApplicationInstruction => AdditionalContextKind::Application,
        }
    }
}

/// Suppresses unchanged rendered publications without rewriting earlier messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdditionalContextStore {
    snapshot: AdditionalContextSnapshot,
}

impl AdditionalContextStore {
    pub fn merge(&mut self, action: AdditionalContextAction) -> Vec<ResponseItem> {
        let values = match action {
            AdditionalContextAction::KeepSourceState => return Vec::new(),
            AdditionalContextAction::PublishSnapshot(values) => values,
            AdditionalContextAction::ClearSourceState => BTreeMap::new(),
        };
        let (items, snapshot) = self.prepare(values);
        self.commit(snapshot);
        items
    }

    pub fn prepare(
        &self,
        values: BTreeMap<String, AdditionalContextEntry>,
    ) -> (Vec<ResponseItem>, AdditionalContextSnapshot) {
        let mut fragments = Vec::new();
        let mut next_entries = BTreeMap::new();

        for (key, entry) in values {
            let treatment = AdditionalContextTreatment::from_kind(entry.kind);
            let fragment: Box<dyn ContextualUserFragment> = match entry.kind {
                AdditionalContextKind::Untrusted => {
                    Box::new(AdditionalContextUserFragment::new(key.clone(), entry.value))
                }
                AdditionalContextKind::Application => Box::new(
                    AdditionalContextDeveloperFragment::new(key.clone(), entry.value),
                ),
            };
            let value_fingerprint = ContextFingerprint::for_rendered(&fragment.render());
            let next_entry = AdditionalContextSnapshotEntry {
                treatment,
                value_fingerprint,
            };
            if self.snapshot.entries.get(&key) != Some(&next_entry) {
                fragments.push(ResponseItem::from(fragment.render_fragment()));
            }
            next_entries.insert(key, next_entry);
        }

        (
            fragments,
            AdditionalContextSnapshot {
                entries: next_entries,
            },
        )
    }

    pub fn commit(&mut self, snapshot: AdditionalContextSnapshot) {
        self.snapshot = snapshot;
    }

    pub fn snapshot(&self) -> AdditionalContextSnapshot {
        self.snapshot.clone()
    }

    pub fn restore(&mut self, snapshot: AdditionalContextSnapshot) {
        self.snapshot = snapshot;
    }

    pub fn current_keys_and_kinds(&self) -> Vec<(String, AdditionalContextKind)> {
        self.snapshot
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.treatment.kind()))
            .collect()
    }
}

#[cfg(test)]
#[path = "additional_context_tests.rs"]
mod tests;
