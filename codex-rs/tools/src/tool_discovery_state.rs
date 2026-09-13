//! Surviving schema visibility and restoration of a search awaiting continuation.

use std::collections::HashSet;

use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use codex_utils_cache::sha1_digest;
use serde::Serialize;
use serde_json::Value;

use crate::response_history::is_model_generated_item;

/// Tracks schemas retained in history and the search awaiting model continuation.
#[derive(Clone, Debug, Default)]
pub struct ToolDiscoveryState {
    // Keep one fingerprint per retained definition. History replacement rebuilds
    // this index, so eviction cannot republish a schema that is still visible.
    fingerprints: HashSet<[u8; 20]>,
    continuation: SearchContinuation,
}

#[derive(Clone, Debug, Default)]
enum SearchContinuation {
    #[default]
    ContinueCurrentContext,
    RehydrateSearch {
        call_id: NonEmptyString,
        output: Box<ResponseItem>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NonEmptyString(String);

#[derive(Debug, thiserror::Error)]
#[error("a deferred search requires a nonempty call ID")]
struct SearchCallIdentityRequired;

impl NonEmptyString {
    fn parse(value: &str) -> Result<Self, SearchCallIdentityRequired> {
        if value.is_empty() {
            Err(SearchCallIdentityRequired)
        } else {
            Ok(Self(value.to_owned()))
        }
    }
}

enum DefinitionVisibility {
    RenderDefinition,
    ReuseDefinition,
}

impl SearchContinuation {
    fn observe_search_output(&mut self, item: &ResponseItem) {
        if let ResponseItem::ToolSearchOutput {
            call_id: Some(call_id),
            execution,
            ..
        } = item
            && execution == "client"
        {
            *self = match NonEmptyString::parse(call_id) {
                Ok(call_id) => Self::RehydrateSearch {
                    call_id,
                    output: Box::new(item.clone()),
                },
                Err(_) => Self::ContinueCurrentContext,
            };
        }
    }

    fn restore_from(&mut self, previous: &Self) {
        match (self, previous) {
            (
                Self::RehydrateSearch { call_id, output },
                Self::RehydrateSearch {
                    call_id: previous_id,
                    output: previous_output,
                },
            ) => {
                if call_id == previous_id {
                    *output = previous_output.clone();
                }
            }
            (
                Self::ContinueCurrentContext,
                Self::ContinueCurrentContext | Self::RehydrateSearch { .. },
            )
            | (Self::RehydrateSearch { .. }, Self::ContinueCurrentContext) => {}
        }
    }

    fn pending_exchange(&self, items: &[ResponseItemEnvelope]) -> Vec<ResponseItemEnvelope> {
        match self {
            Self::ContinueCurrentContext => Vec::new(),
            Self::RehydrateSearch { call_id, output: full_output } => {
                match (
                    items.iter().rev().find(|item| matches!(&item.item, ResponseItem::ToolSearchCall { call_id: Some(id), execution, .. } if id == &call_id.0 && execution == "client")),
                    items.iter().rev().find(|item| matches!(&item.item, ResponseItem::ToolSearchOutput { call_id: Some(id), execution, .. } if id == &call_id.0 && execution == "client")),
                ) {
                    (Some(call), Some(output)) => {
                        let mut output = output.clone();
                        output.item = full_output.as_ref().clone();
                        vec![call.clone(), output]
                    }
                    _ => Vec::new(),
                }
            }
        }
    }
}

impl ToolDiscoveryState {
    fn note_model_generated_item(&mut self) {
        self.continuation = SearchContinuation::ContinueCurrentContext;
    }

    pub fn observe(&mut self, item: &mut ResponseItem) {
        if is_model_generated_item(item)
            && !matches!(
                item,
                ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. }
            )
        {
            self.note_model_generated_item();
        }
        if let ResponseItem::ToolSearchOutput { tools, .. } = item {
            *tools = crate::bound_tool_search_output(std::mem::take(tools));
        }
        self.deduplicate_response_item(item);
    }

    fn deduplicate_response_item(&mut self, item: &mut ResponseItem) {
        self.continuation.observe_search_output(item);
        let ResponseItem::ToolSearchOutput { tools, .. } = item else {
            return;
        };
        let mut retained = Vec::with_capacity(tools.len());
        for tool in std::mem::take(tools) {
            match tool.as_object().filter(|namespace| {
                namespace.get("type").and_then(Value::as_str) == Some("namespace")
            }) {
                Some(namespace) => match namespace
                    .get("tools")
                    .and_then(Value::as_array)
                    .filter(|tools| !tools.is_empty())
                {
                    Some(leaves) => {
                        let mut metadata = namespace.clone();
                        metadata.remove("tools");
                        let leaves = leaves
                            .iter()
                            .filter(|leaf| {
                                matches!(
                                    self.remember_serialized(&(&metadata, leaf)),
                                    DefinitionVisibility::RenderDefinition
                                )
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        if !leaves.is_empty() {
                            metadata.insert("tools".to_string(), Value::Array(leaves));
                            retained.push(Value::Object(metadata));
                        }
                    }
                    None => match self.remember_serialized(&tool) {
                        DefinitionVisibility::RenderDefinition => retained.push(tool),
                        DefinitionVisibility::ReuseDefinition => {}
                    },
                },
                None => match self.remember_serialized(&tool) {
                    DefinitionVisibility::RenderDefinition => retained.push(tool),
                    DefinitionVisibility::ReuseDefinition => {}
                },
            }
        }
        *tools = retained;
    }

    pub fn rebuild(&mut self, items: &[ResponseItemEnvelope]) {
        let mut rebuilt = Self::default();
        for envelope in items {
            rebuilt.observe(&mut envelope.item.clone());
        }
        rebuilt.restore_pending_output_from(self);
        *self = rebuilt;
    }

    pub fn restore_pending_output_from(&mut self, previous: &Self) {
        self.continuation.restore_from(&previous.continuation);
    }

    pub fn pending_exchange(
        &self,
        items: &[ResponseItemEnvelope],
    ) -> Vec<ResponseItemEnvelope> {
        self.continuation.pending_exchange(items)
    }

    fn remember_serialized(&mut self, value: &impl Serialize) -> DefinitionVisibility {
        let fingerprint = match serde_json::to_vec(value) {
            Ok(serialized) => sha1_digest(&serialized),
            Err(_) => return DefinitionVisibility::RenderDefinition,
        };
        if !self.fingerprints.insert(fingerprint) {
            return DefinitionVisibility::ReuseDefinition;
        }
        DefinitionVisibility::RenderDefinition
    }
}

/// Removes schema bodies while retaining each search output's envelope.
pub fn strip_tool_search_schemas<'a>(
    items: impl IntoIterator<Item = &'a mut ResponseItem>,
) -> usize {
    let mut stripped = 0;
    for item in items {
        if let ResponseItem::ToolSearchOutput { tools, .. } = item
            && !tools.is_empty()
        {
            tools.clear();
            stripped += 1;
        }
    }
    stripped
}

#[cfg(test)]
#[path = "tool_discovery_state_tests.rs"]
mod tests;
