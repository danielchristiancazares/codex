use codex_connectors::AppInfo;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const TUI_CLIENT_NAME: &str = "codex-tui";
pub const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
pub const TOOL_SEARCH_DEFAULT_LIMIT: usize = 8;
/// Maximum number of leaf definitions returned by one deferred-tool search.
pub const TOOL_SEARCH_MAX_RESULTS: usize = 32;
/// Maximum serialized size of the `tools` array returned by deferred-tool search.
pub const TOOL_SEARCH_MAX_OUTPUT_BYTES: usize = 32 * 1024;
pub const LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME: &str = "list_available_plugins_to_install";
pub const REQUEST_PLUGIN_INSTALL_TOOL_NAME: &str = "request_plugin_install";

/// Retains whole deferred-tool definitions within the shared count and byte ceilings.
pub fn bound_tool_search_output(tools: Vec<Value>) -> Vec<Value> {
    let mut retained = Vec::new();
    let mut leaf_count = 0usize;

    for tool in tools {
        if leaf_count >= TOOL_SEARCH_MAX_RESULTS {
            break;
        }
        let namespace_tools = tool
            .get("type")
            .and_then(Value::as_str)
            .filter(|tool_type| *tool_type == "namespace")
            .and_then(|_| tool.get("tools"))
            .and_then(Value::as_array);
        let Some(namespace_tools) = namespace_tools else {
            let mut candidate = retained.clone();
            candidate.push(tool);
            if serde_json::to_vec(&candidate)
                .is_ok_and(|serialized| serialized.len() <= TOOL_SEARCH_MAX_OUTPUT_BYTES)
            {
                retained = candidate;
                leaf_count += 1;
                continue;
            }
            break;
        };

        let Some(mut namespace) = tool.as_object().cloned() else {
            continue;
        };
        namespace.insert("tools".to_string(), Value::Array(Vec::new()));
        let mut bounded_tools = Vec::new();
        for namespace_tool in namespace_tools {
            if leaf_count >= TOOL_SEARCH_MAX_RESULTS {
                break;
            }
            bounded_tools.push(namespace_tool.clone());
            namespace.insert("tools".to_string(), Value::Array(bounded_tools.clone()));
            let mut candidate = retained.clone();
            candidate.push(Value::Object(namespace.clone()));
            if serde_json::to_vec(&candidate)
                .is_ok_and(|serialized| serialized.len() <= TOOL_SEARCH_MAX_OUTPUT_BYTES)
            {
                leaf_count += 1;
            } else {
                bounded_tools.pop();
                break;
            }
        }
        if !bounded_tools.is_empty() {
            namespace.insert("tools".to_string(), Value::Array(bounded_tools));
            retained.push(Value::Object(namespace));
        }
    }

    retained
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolSearchSourceInfo {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolType {
    Connector,
    Plugin,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolAction {
    Install,
    Enable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiscoverableTool {
    Connector(Box<AppInfo>),
    Plugin(Box<DiscoverablePluginInfo>),
}

impl DiscoverableTool {
    pub fn tool_type(&self) -> DiscoverableToolType {
        match self {
            Self::Connector(_) => DiscoverableToolType::Connector,
            Self::Plugin(_) => DiscoverableToolType::Plugin,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Connector(connector) => connector.id.as_str(),
            Self::Plugin(plugin) => plugin.id.as_str(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Connector(connector) => connector.name.as_str(),
            Self::Plugin(plugin) => plugin.name.as_str(),
        }
    }

    pub fn install_url(&self) -> Option<&str> {
        match self {
            Self::Connector(connector) => connector.install_url.as_deref(),
            Self::Plugin(_) => None,
        }
    }
}

impl From<AppInfo> for DiscoverableTool {
    fn from(value: AppInfo) -> Self {
        Self::Connector(Box::new(value))
    }
}

impl From<DiscoverablePluginInfo> for DiscoverableTool {
    fn from(value: DiscoverablePluginInfo) -> Self {
        Self::Plugin(Box::new(value))
    }
}

pub fn filter_request_plugin_install_discoverable_tools_for_client(
    discoverable_tools: Vec<DiscoverableTool>,
    app_server_client_name: Option<&str>,
) -> Vec<DiscoverableTool> {
    if app_server_client_name != Some(TUI_CLIENT_NAME) {
        return discoverable_tools;
    }

    discoverable_tools
        .into_iter()
        .filter(|tool| !matches!(tool, DiscoverableTool::Plugin(_)))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoverablePluginInfo {
    pub id: String,
    pub remote_plugin_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tool_type: DiscoverableToolType,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ListAvailablePluginsToInstallResult {
    pub tools: Vec<RequestPluginInstallEntry>,
}

pub fn collect_request_plugin_install_entries(
    discoverable_tools: &[DiscoverableTool],
) -> Vec<RequestPluginInstallEntry> {
    discoverable_tools
        .iter()
        .map(|tool| match tool {
            DiscoverableTool::Connector(connector) => RequestPluginInstallEntry {
                id: connector.id.clone(),
                name: connector.name.clone(),
                description: connector.description.clone(),
                tool_type: DiscoverableToolType::Connector,
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: Vec::new(),
            },
            DiscoverableTool::Plugin(plugin) => RequestPluginInstallEntry {
                id: plugin.id.clone(),
                name: plugin.name.clone(),
                description: plugin.description.clone(),
                tool_type: DiscoverableToolType::Plugin,
                has_skills: plugin.has_skills,
                mcp_server_names: plugin.mcp_server_names.clone(),
                app_connector_ids: plugin.app_connector_ids.clone(),
            },
        })
        .collect()
}

#[cfg(test)]
#[path = "tool_discovery_tests.rs"]
mod tests;
