use std::collections::HashSet;

use codex_protocol::models::ResponseItem;
use codex_utils_cache::sha1_digest;
use serde::Serialize;
use serde_json::Value;

const MAX_TRACKED_TOOL_DEFINITIONS: usize = 1_024;

/// Bounded fingerprints for deferred-tool definitions already retained in history.
#[derive(Clone, Debug, Default)]
pub(super) struct ToolDiscoveryState {
    fingerprints: HashSet<[u8; 20]>,
}

impl ToolDiscoveryState {
    pub(super) fn deduplicate_response_item(&mut self, item: &mut ResponseItem) {
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
    }

    fn remember_serialized(&mut self, value: &impl Serialize) -> bool {
        let Some(fingerprint) = fingerprint(value) else {
            return true;
        };
        if self.fingerprints.contains(&fingerprint) {
            return false;
        }
        if self.fingerprints.len() < MAX_TRACKED_TOOL_DEFINITIONS {
            self.fingerprints.insert(fingerprint);
        }
        true
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
