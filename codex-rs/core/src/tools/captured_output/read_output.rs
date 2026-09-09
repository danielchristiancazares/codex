use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_utils_output_truncation::CaptureId;
use codex_utils_output_truncation::CaptureQuery;
use codex_utils_output_truncation::CaptureRange;
use codex_utils_output_truncation::SearchText;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) struct ReadOutputHandler;

struct CaptureRequest {
    id: CaptureId,
    query: CaptureQuery,
}

impl<'de> Deserialize<'de> for CaptureRequest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut fields = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let id = serde_json::from_value::<uuid::Uuid>(
            fields
                .remove("output_id")
                .ok_or_else(|| serde::de::Error::custom("output_id is required"))?,
        )
        .map_err(serde::de::Error::custom)?;
        let query = match fields
            .remove("query")
            .and_then(|value| value.as_str().map(str::to_owned))
            .as_deref()
        {
            Some("range") => {
                let start = serde_json::from_value::<usize>(
                    fields
                        .remove("start_byte")
                        .ok_or_else(|| serde::de::Error::custom("start_byte is required"))?,
                )
                .map_err(serde::de::Error::custom)?;
                let end = serde_json::from_value::<usize>(
                    fields
                        .remove("end_byte")
                        .ok_or_else(|| serde::de::Error::custom("end_byte is required"))?,
                )
                .map_err(serde::de::Error::custom)?;
                CaptureQuery::Range(
                    CaptureRange::new(start, end).map_err(serde::de::Error::custom)?,
                )
            }
            Some("search") => {
                let text = serde_json::from_value::<String>(
                    fields
                        .remove("text")
                        .ok_or_else(|| serde::de::Error::custom("text is required"))?,
                )
                .map_err(serde::de::Error::custom)?;
                CaptureQuery::Search(SearchText::try_from(text).map_err(serde::de::Error::custom)?)
            }
            _ => return Err(serde::de::Error::custom("query must be range or search")),
        };
        if !fields.is_empty() {
            return Err(serde::de::Error::custom("unexpected output query fields"));
        }
        Ok(Self {
            id: CaptureId::new(id.into_bytes()),
            query,
        })
    }
}

impl ToolExecutor<ToolInvocation> for ReadOutputHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("read_output")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: "read_output".to_string(),
            description: "Retrieve retained terminal or Code Mode output without rerunning work. Results are capped at 3200 bytes. Captures are session-local and oldest captures expire beyond 32 entries or 32 MiB.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([
                    ("output_id".to_string(), JsonSchema::string(Some("ID from a captured-output receipt.".to_string()))),
                    ("query".to_string(), JsonSchema::string(Some("range or search.".to_string()))),
                    ("start_byte".to_string(), JsonSchema::number(Some("For range, zero-based start byte.".to_string()))),
                    ("end_byte".to_string(), JsonSchema::number(Some("For range, exclusive end byte, at most 2800 bytes after start.".to_string()))),
                    ("text".to_string(), JsonSchema::string(Some("For search, a nonempty literal of at most 512 bytes.".to_string()))),
                ]),
                Some(vec!["output_id".to_string(), "query".to_string()]),
                Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async move {
            let ToolPayload::Function { arguments } = invocation.payload else {
                return Err(FunctionCallError::RespondToModel(
                    "read_output expects JSON arguments".into(),
                ));
            };
            let request: CaptureRequest = super::super::handlers::parse_arguments(&arguments)?;
            let text = invocation
                .session
                .services
                .captured_output
                .lock()
                .await
                .read(&request.id, request.query)
                .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
            Ok(boxed_tool_output(FunctionToolOutput::from_text(
                text.to_string(),
                Some(true),
            )))
        })
    }
}

impl CoreToolRuntime for ReadOutputHandler {
    fn pre_tool_use_payload(&self, _invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        None
    }

    fn post_tool_use_payload(
        &self,
        _invocation: &ToolInvocation,
        _result: &dyn crate::tools::context::ToolOutput,
    ) -> Option<PostToolUsePayload> {
        None
    }
}

#[cfg(test)]
#[path = "read_output_tests.rs"]
mod tests;
