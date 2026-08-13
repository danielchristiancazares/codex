use codex_config::McpServerTransportConfig;
use codex_git_utils::get_git_repo_root;
use codex_mcp::McpConfig;
use codex_protocol::protocol::McpInvocation;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value;

/// Returns whether an otherwise unknown local MCP read is contained by the
/// active Git repository and can skip automatic review.
pub(super) fn approval_not_required_for_repo_read(
    config: &McpConfig,
    project_cwd: &AbsolutePathBuf,
    invocation: &McpInvocation,
) -> bool {
    (|| -> Result<bool, ()> {
        let server = config
            .mcp_server_catalog
            .server(&invocation.server)
            .ok_or(())?;
        if !server.config().is_local_environment() {
            return Ok(false);
        }
        let tool_cwd = match &server.config().transport {
            McpServerTransportConfig::Stdio { cwd, .. } => {
                if cwd.is_some() {
                    cwd.as_ref()
                        .and_then(|cwd| cwd.to_inferred_abs_path())
                        .ok_or(())?
                } else {
                    project_cwd.clone()
                }
            }
            McpServerTransportConfig::StreamableHttp { .. } => return Ok(false),
        };

        let arguments = invocation
            .arguments
            .as_ref()
            .and_then(Value::as_object)
            .ok_or(())?;
        let path = match invocation.tool.as_str() {
            "Read" | "Outline" | "ListDir" => {
                arguments.get("path").and_then(Value::as_str).ok_or(())?
            }
            "Search" | "search_context" => {
                if arguments
                    .get("follow")
                    .is_some_and(|follow| follow != &Value::Bool(false))
                {
                    return Ok(false);
                }
                arguments
                    .get("path")
                    .map_or(Ok("."), |path| path.as_str().ok_or(()))?
            }
            "Glob" | "CountLines" => arguments
                .get("path")
                .map_or(Ok("."), |path| path.as_str().ok_or(()))?,
            _ => return Ok(false),
        };

        let repo_root = get_git_repo_root(project_cwd).ok_or(())?;
        let repo_root = dunce::canonicalize(repo_root).map_err(|_| ())?;
        let resolved_path = tool_cwd.join(path).canonicalize().map_err(|_| ())?;
        Ok(resolved_path.starts_with(repo_root))
    })()
    .unwrap_or(false)
}

#[cfg(test)]
#[path = "repo_read_approval_tests.rs"]
mod tests;
