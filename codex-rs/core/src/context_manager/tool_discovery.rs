use std::collections::HashSet;
use std::collections::VecDeque;

use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use codex_utils_cache::sha1_digest;
use serde::Serialize;
use serde_json::Value;

const MAX_TRACKED_TOOL_DEFINITIONS: usize = 1_024;

/// Bounded deferred-tool fingerprints plus the latest result awaiting model continuation.
#[derive(Clone, Debug, Default)]
pub(super) struct ToolDiscoveryState {
    fingerprints: HashSet<[u8; 20]>,
    fingerprint_order: VecDeque<[u8; 20]>,
    pending_output: Option<ResponseItem>,
}

impl ToolDiscoveryState {
    pub(super) fn note_model_generated_item(&mut self) {
        self.pending_output = None;
    }

    pub(super) fn deduplicate_response_item(&mut self, item: &mut ResponseItem) {
        let pending_output = matches!(
            item,
            ResponseItem::ToolSearchOutput {
                call_id: Some(_),
                execution,
                ..
            } if execution == "client"
        )
        .then(|| item.clone());
        let ResponseItem::ToolSearchOutput { tools, .. } = item else {
            return;
        };

        let mut retained = Vec::with_capacity(tools.len());
        for tool in std::mem::take(tools) {
            let Some(namespace) = tool.as_object().filter(|namespace| {
                namespace.get("type").and_then(Value::as_str) == Some("namespace")
            }) else {
                if self.remember_serialized(&tool) {
                    retained.push(tool);
                }
                continue;
            };
            let Some(namespace_tools) = namespace.get("tools").and_then(Value::as_array) else {
                if self.remember_serialized(&tool) {
                    retained.push(tool);
                }
                continue;
            };
            if namespace_tools.is_empty() {
                if self.remember_serialized(&tool) {
                    retained.push(tool);
                }
                continue;
            }

            let mut namespace_metadata = namespace.clone();
            namespace_metadata.remove("tools");
            let namespace_metadata = Value::Object(namespace_metadata);
            let retained_tools = namespace_tools
                .iter()
                .filter(|namespace_tool| {
                    self.remember_serialized(&(&namespace_metadata, namespace_tool))
                })
                .cloned()
                .collect::<Vec<_>>();
            if retained_tools.is_empty() {
                continue;
            }

            let mut namespace = namespace.clone();
            namespace.insert("tools".to_string(), Value::Array(retained_tools));
            retained.push(Value::Object(namespace));
        }
        *tools = retained;
        if let Some(pending_output) = pending_output {
            self.pending_output = Some(pending_output);
        }
    }

    pub(super) fn restore_pending_output_from(&mut self, previous: &Self) {
        let Some(previous_output) = previous.pending_output.as_ref() else {
            return;
        };
        let Some(previous_call_id) = tool_search_output_call_id(previous_output) else {
            return;
        };
        if self
            .pending_output
            .as_ref()
            .and_then(tool_search_output_call_id)
            == Some(previous_call_id)
        {
            self.pending_output = Some(previous_output.clone());
        }
    }

    pub(super) fn pending_exchange(
        &self,
        items: &[ResponseItemEnvelope],
    ) -> Vec<ResponseItemEnvelope> {
        let Some(pending_output) = self.pending_output.as_ref() else {
            return Vec::new();
        };
        let Some(call_id) = tool_search_output_call_id(pending_output) else {
            return Vec::new();
        };
        let Some(call) = items.iter().rev().find(|envelope| {
            matches!(
                &envelope.item,
                ResponseItem::ToolSearchCall {
                    call_id: Some(existing_call_id),
                    ..
                } if existing_call_id == call_id
            )
        }) else {
            return Vec::new();
        };
        let Some(output) = items.iter().rev().find(|envelope| {
            matches!(
                &envelope.item,
                ResponseItem::ToolSearchOutput {
                    call_id: Some(existing_call_id),
                    ..
                } if existing_call_id == call_id
            )
        }) else {
            return Vec::new();
        };
        let mut output = output.clone();
        output.item = pending_output.clone();
        vec![call.clone(), output]
    }

    fn remember_serialized(&mut self, value: &impl Serialize) -> bool {
        let Some(fingerprint) = fingerprint(value) else {
            return true;
        };
        if self.fingerprints.contains(&fingerprint) {
            return false;
        }
        self.fingerprints.insert(fingerprint);
        self.fingerprint_order.push_back(fingerprint);
        if self.fingerprints.len() > MAX_TRACKED_TOOL_DEFINITIONS
            && let Some(expired) = self.fingerprint_order.pop_front()
        {
            self.fingerprints.remove(&expired);
        }
        true
    }
}

pub(crate) fn strip_tool_search_schemas<'a>(
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

fn tool_search_output_call_id(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::ToolSearchOutput {
            call_id: Some(call_id),
            execution,
            ..
        } if execution == "client" => Some(call_id),
        _ => None,
    }
}

fn fingerprint(value: &impl Serialize) -> Option<[u8; 20]> {
    serde_json::to_vec(value)
        .ok()
        .map(|serialized| sha1_digest(&serialized))
}

#[cfg(test)]
#[path = "tool_discovery_tests.rs"]
mod tests;
