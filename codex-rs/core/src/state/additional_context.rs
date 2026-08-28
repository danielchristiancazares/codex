use std::collections::BTreeMap;

use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;

use crate::context::AdditionalContextDeveloperFragment;
use crate::context::AdditionalContextUserFragment;
use crate::context::ContextualUserFragment;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AdditionalContextSnapshot {
    entries: BTreeMap<String, AdditionalContextSnapshotEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AdditionalContextSnapshotEntry {
    treatment: AdditionalContextTreatment,
    value_fingerprint: String,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    snapshot: AdditionalContextSnapshot,
}

impl AdditionalContextStore {
    pub(crate) fn prepare(
        &self,
        values: BTreeMap<String, AdditionalContextEntry>,
    ) -> (Vec<ResponseItem>, AdditionalContextSnapshot) {
        let next = AdditionalContextSnapshot {
            entries: values
                .iter()
                .map(|(key, entry)| {
                    (
                        key.clone(),
                        AdditionalContextSnapshotEntry {
                            treatment: AdditionalContextTreatment::from_kind(entry.kind),
                            value_fingerprint: value_fingerprint(&entry.value),
                        },
                    )
                })
                .collect(),
        };
        let fragments = values
            .into_iter()
            .filter(|(key, _)| self.snapshot.entries.get(key) != next.entries.get(key))
            .map(|(key, entry)| match entry.kind {
                AdditionalContextKind::Untrusted => ContextualUserFragment::into(
                    AdditionalContextUserFragment::new(key, entry.value),
                ),
                AdditionalContextKind::Application => ContextualUserFragment::into(
                    AdditionalContextDeveloperFragment::new(key, entry.value),
                ),
            })
            .collect();
        (fragments, next)
    }

    pub(crate) fn commit(&mut self, snapshot: AdditionalContextSnapshot) {
        self.snapshot = snapshot;
    }

    pub(crate) fn snapshot(&self) -> AdditionalContextSnapshot {
        self.snapshot.clone()
    }

    pub(crate) fn restore(&mut self, snapshot: AdditionalContextSnapshot) {
        self.snapshot = snapshot;
    }

    pub(crate) fn current_keys_and_kinds(&self) -> Vec<(String, AdditionalContextKind)> {
        self.snapshot
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.treatment.kind()))
            .collect()
    }
}

fn value_fingerprint(value: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b"codex-additional-context-v1\0");
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "additional_context_tests.rs"]
mod tests;
