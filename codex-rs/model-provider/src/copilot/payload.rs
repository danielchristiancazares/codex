use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Initiator {
    Agent,
    User,
}

impl Initiator {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::User => "user",
        }
    }

    pub(super) const fn interaction_type(self) -> &'static str {
        match self {
            Self::Agent => "conversation-agent",
            Self::User => "conversation-user",
        }
    }
}

pub(super) fn normalize_websocket(payload: &mut Value) {
    normalize_common(payload);
}

fn normalize_common(payload: &mut Value) {
    if let Some(object) = payload.as_object_mut() {
        object.insert("service_tier".to_string(), Value::Null);
    }
    replace_apply_patch_tool(payload);
    remove_web_search_tools(payload);
    retain_latest_compaction(payload);
}

pub(super) fn initiator(payload: &Value) -> Initiator {
    let Some(last_item) = payload
        .get("input")
        .and_then(Value::as_array)
        .and_then(|items| items.last())
    else {
        return Initiator::User;
    };

    match last_item
        .get("role")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("assistant") | None => Initiator::Agent,
        Some(_) => Initiator::User,
    }
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

fn replace_apply_patch_tool(payload: &mut Value) {
    let Some(tools) = payload.get_mut("tools").and_then(Value::as_array_mut) else {
        return;
    };
    for tool in tools {
        if tool.get("type").and_then(Value::as_str) == Some("custom")
            && tool.get("name").and_then(Value::as_str) == Some("apply_patch")
        {
            *tool = serde_json::json!({
                "type": "function",
                "name": "apply_patch",
                "description": "Use the apply_patch tool to edit files",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "input": {
                            "type": "string",
                            "description": "The entire contents of the apply_patch command"
                        }
                    },
                    "required": ["input"]
                },
                "strict": false
            });
        }
    }
}

#[cfg(test)]
#[path = "payload_tests.rs"]
mod tests;
