use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::mcp_resource_spec::create_read_mcp_resource_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_function_output_items_with_policy;

use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ResourceContents;

use super::ReadResourceArgs;
use super::ReadResourcePayload;
use super::ensure_model_can_access_mcp_server;
use super::normalize_required_string;
use super::parse_args;
use super::parse_arguments;
use super::run_resource_operation;

fn resource_header(server: &str, uri: &str, mime_type: Option<&str>) -> String {
    match mime_type {
        Some(mime_type) => format!("MCP resource `{uri}` from `{server}` ({mime_type})"),
        None => format!("MCP resource `{uri}` from `{server}`"),
    }
}

fn normalized_mime_type(mime_type: Option<&str>) -> Option<&str> {
    mime_type
        .and_then(|mime_type| mime_type.split(';').next())
        .map(str::trim)
        .filter(|mime_type| !mime_type.is_empty())
}

fn media_data_url(data: String, mime_type: &str) -> String {
    if data.starts_with("data:") {
        data
    } else {
        format!("data:{mime_type};base64,{data}")
    }
}

pub(super) fn project_read_resource_output(
    payload: ReadResourcePayload,
    truncation_policy: TruncationPolicy,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let ReadResourcePayload {
        server,
        uri: requested_uri,
        result,
    } = payload;
    let mut output = Vec::new();
    for content in result.contents {
        match content {
            ResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
                ..
            } => {
                let header = resource_header(&server, &uri, mime_type.as_deref());
                output.push(FunctionCallOutputContentItem::InputText {
                    text: format!("{header}\n{text}"),
                });
            }
            ResourceContents::BlobResourceContents {
                uri,
                mime_type,
                blob,
                ..
            } => {
                let normalized_mime_type = normalized_mime_type(mime_type.as_deref());
                output.push(FunctionCallOutputContentItem::InputText {
                    text: resource_header(&server, &uri, mime_type.as_deref()),
                });
                match normalized_mime_type {
                    Some(mime_type) if mime_type.to_ascii_lowercase().starts_with("image/") => {
                        output.push(FunctionCallOutputContentItem::InputImage {
                            image_url: media_data_url(blob, mime_type),
                            detail: Some(DEFAULT_IMAGE_DETAIL),
                        });
                    }
                    Some(mime_type) if mime_type.to_ascii_lowercase().starts_with("audio/") => {
                        output.push(FunctionCallOutputContentItem::InputAudio {
                            audio_url: media_data_url(blob, mime_type),
                        });
                    }
                    _ => output.push(FunctionCallOutputContentItem::InputText {
                        text: format!("[binary payload omitted: {} base64 characters]", blob.len()),
                    }),
                }
            }
            _ => output.push(FunctionCallOutputContentItem::InputText {
                text: format!(
                    "MCP resource `{requested_uri}` from `{server}` returned an unsupported content type"
                ),
            }),
        }
    }
    if output.is_empty() {
        output.push(FunctionCallOutputContentItem::InputText {
            text: format!("MCP resource `{requested_uri}` from `{server}` returned no content"),
        });
    }
    let output = truncate_function_output_items_with_policy(
        &output,
        truncation_policy,
        crate::context_manager::estimate_function_output_content_item_tokens,
    );
    Ok(FunctionToolOutput::from_content(output, Some(true)))
}

pub struct ReadMcpResourceHandler;

impl ToolExecutor<ToolInvocation> for ReadMcpResourceHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("read_mcp_resource")
    }

    fn spec(&self) -> ToolSpec {
        create_read_mcp_resource_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl ReadMcpResourceHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            step_context,
            call_id,
            payload,
            ..
        } = invocation;
        let turn = std::sync::Arc::clone(&step_context.turn);
        let mcp = &step_context.mcp;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "read_mcp_resource handler received unsupported payload".to_string(),
                ));
            }
        };

        let arguments = parse_arguments(arguments.as_str())?;
        let args: ReadResourceArgs = parse_args(arguments.clone())?;
        let ReadResourceArgs { server, uri } = args;
        let server = normalize_required_string("server", server)?;
        let uri = normalize_required_string("uri", uri)?;

        let invocation = McpInvocation {
            server: server.clone(),
            tool: "read_mcp_resource".to_string(),
            arguments: arguments.clone(),
        };

        run_resource_operation(
            &session,
            turn.as_ref(),
            &call_id,
            invocation,
            async {
                ensure_model_can_access_mcp_server(turn.as_ref(), &server)?;
                let result = mcp
                    .read_resource(&server, ReadResourceRequestParams::new(uri.clone()))
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!("resources/read failed: {err:#}"))
                    })?;

                Ok(ReadResourcePayload {
                    server,
                    uri,
                    result,
                })
            },
            project_read_resource_output,
        )
        .await
    }
}

impl CoreToolRuntime for ReadMcpResourceHandler {}
