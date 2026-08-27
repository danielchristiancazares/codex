use serde_json::Value;

pub(super) fn normalize_websocket(payload: &mut Value) {
    normalize_common(payload);
}

fn normalize_common(payload: &mut Value) {
    if let Some(object) = payload.as_object_mut() {
        for field in [
            "client_metadata",
            "prompt_cache_key",
            "service_tier",
            "stream",
            "stream_options",
            "tool_choice",
        ] {
            object.remove(field);
        }
    }
    remove_web_search_tools(payload);
    retain_latest_compaction(payload);
}

fn retain_latest_compaction(payload: &mut Value) {
    let Some(input) = payload.get_mut("input").and_then(Value::as_array_mut) else {
        return;
    };
    if let Some(index) = input
        .iter()
        .rposition(|item| item.get("type").and_then(Value::as_str) == Some("compaction"))
    {
        *input = input.split_off(index);
    }
}

fn remove_web_search_tools(payload: &mut Value) {
    let Some(tools) = payload.get_mut("tools").and_then(Value::as_array_mut) else {
        return;
    };
    tools.retain(|tool| {
        !matches!(
            tool.get("type").and_then(Value::as_str),
            Some("web_search" | "web_search_preview")
        )
    });
}

#[cfg(test)]
#[path = "payload_tests.rs"]
mod tests;
