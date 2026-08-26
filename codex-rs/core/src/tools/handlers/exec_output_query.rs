use codex_exec_output_artifacts::ArtifactError;
use codex_exec_output_artifacts::ArtifactQuery;
use codex_exec_output_artifacts::ArtifactQueryData;
use codex_exec_output_artifacts::ArtifactQueryPresentation;
use codex_exec_output_artifacts::ArtifactSearchMode;
use codex_exec_output_artifacts::ArtifactStore;
use codex_otel::EXEC_OUTPUT_ARTIFACT_AUTHORIZATION_FAILURE_METRIC;
use codex_otel::EXEC_OUTPUT_ARTIFACT_CORRUPTION_FAILURE_METRIC;
use codex_otel::EXEC_OUTPUT_ARTIFACT_PRESENTED_OUTPUT_BYTES_METRIC;
use codex_otel::EXEC_OUTPUT_ARTIFACT_PREVIEW_TRUNCATION_METRIC;
use codex_otel::EXEC_OUTPUT_ARTIFACT_QUERY_COUNT_METRIC;
use codex_otel::SessionTelemetry;
use codex_tools::JsonToolOutput;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::exec_output_query_spec::EXEC_OUTPUT_QUERY_TOOL_NAME;
use crate::tools::handlers::exec_output_query_spec::create_exec_output_query_tool;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use crate::unified_exec::exec_output_artifact_access;

const DEFAULT_QUERY_OUTPUT_BYTES: usize = 8 * 1024;

#[derive(Debug, Deserialize)]
struct ExecOutputQueryArgs {
    artifact_ref: String,
    view: ArtifactQueryView,
    #[serde(default)]
    environment_id: Option<String>,
    #[serde(default)]
    max_output_bytes: Option<usize>,
    #[serde(default)]
    start: Option<usize>,
    #[serde(default)]
    end: Option<usize>,
    #[serde(default)]
    length: Option<usize>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    search_mode: Option<ArtifactSearchModeArg>,
    #[serde(default)]
    case_sensitive: Option<bool>,
    #[serde(default)]
    context_lines: Option<usize>,
    #[serde(default)]
    max_matches: Option<usize>,
    #[serde(default)]
    include_data: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ArtifactQueryView {
    Metadata,
    Head,
    Tail,
    Bytes,
    Lines,
    Search,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ArtifactSearchModeArg {
    Literal,
    Regex,
}

pub struct ExecOutputQueryHandler;

impl ToolExecutor<ToolInvocation> for ExecOutputQueryHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(EXEC_OUTPUT_QUERY_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_exec_output_query_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            let ToolPayload::Function { arguments } = &invocation.payload else {
                return Err(FunctionCallError::RespondToModel(
                    "exec_output_query handler received unsupported payload".to_string(),
                ));
            };
            let args: ExecOutputQueryArgs = parse_arguments(arguments)?;
            let store = invocation
                .session
                .services
                .thread_extension_data
                .get::<ArtifactStore>()
                .ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "exec-output artifact storage is unavailable in this session".to_string(),
                    )
                })?;
            let environment = resolve_tool_environment(
                &invocation.step_context.environments,
                args.environment_id.as_deref(),
            )?
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "exec-output artifact storage is unavailable in this environment".to_string(),
                )
            })?;
            let access = exec_output_artifact_access(
                invocation.session.thread_id,
                &environment.selection.environment_id,
                environment.selection.workspace_roots.iter(),
            );
            let query = args.to_query()?;
            let artifact_ref = args.artifact_ref.clone();
            let history_version = invocation.session.clone_history().await.history_version();
            let presentation_scope = format!("{}:{history_version}", invocation.turn.sub_id);
            let presentation = if args.include_data {
                ArtifactQueryPresentation::include_data(presentation_scope)
            } else {
                ArtifactQueryPresentation::return_receipt(presentation_scope)
            };
            let telemetry = invocation.session.services.session_telemetry.clone();
            telemetry.counter(EXEC_OUTPUT_ARTIFACT_QUERY_COUNT_METRIC, /*inc*/ 1, &[]);
            let result = tokio::task::spawn_blocking(move || {
                store.query(&artifact_ref, &access, &query, &presentation)
            })
            .await
            .map_err(|_| {
                FunctionCallError::RespondToModel(
                    "exec-output artifact query worker stopped unexpectedly".to_string(),
                )
            })?;
            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    record_query_error(&telemetry, &error);
                    return Err(exec_output_query_error(error));
                }
            };
            record_query_result(&telemetry, &result);
            Ok(boxed_tool_output(JsonToolOutput::new(
                serde_json::to_value(result).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to serialize artifact query result: {err}"
                    ))
                })?,
            )))
        })
    }
}

impl CoreToolRuntime for ExecOutputQueryHandler {}

impl ExecOutputQueryArgs {
    fn to_query(&self) -> Result<ArtifactQuery, FunctionCallError> {
        let max_output_bytes = self.max_output_bytes.unwrap_or(DEFAULT_QUERY_OUTPUT_BYTES);
        match self.view {
            ArtifactQueryView::Metadata => Ok(ArtifactQuery::Metadata),
            ArtifactQueryView::Head => Ok(ArtifactQuery::Head {
                max_bytes: max_output_bytes,
            }),
            ArtifactQueryView::Tail => Ok(ArtifactQuery::Tail {
                max_bytes: max_output_bytes,
            }),
            ArtifactQueryView::Bytes => Ok(ArtifactQuery::Bytes {
                start: required(self.start, "start", "bytes")?,
                length: required(self.length, "length", "bytes")?,
            }),
            ArtifactQueryView::Lines => Ok(ArtifactQuery::Lines {
                start: required(self.start, "start", "lines")?,
                end: required(self.end, "end", "lines")?,
                max_bytes: max_output_bytes,
            }),
            ArtifactQueryView::Search => Ok(ArtifactQuery::Search {
                pattern: self.pattern.clone().ok_or_else(|| {
                    FunctionCallError::RespondToModel(
                        "`pattern` is required for the search view".to_string(),
                    )
                })?,
                mode: match self.search_mode {
                    Some(ArtifactSearchModeArg::Regex) => ArtifactSearchMode::Regex,
                    Some(ArtifactSearchModeArg::Literal) | None => ArtifactSearchMode::Literal,
                },
                case_sensitive: self.case_sensitive.unwrap_or(true),
                context_lines: self.context_lines.unwrap_or(0),
                max_matches: self.max_matches.unwrap_or(20),
                max_bytes: max_output_bytes,
            }),
        }
    }
}

fn required(value: Option<usize>, field: &str, view: &str) -> Result<usize, FunctionCallError> {
    value.ok_or_else(|| {
        FunctionCallError::RespondToModel(format!("`{field}` is required for the {view} view"))
    })
}

fn exec_output_query_error(error: ArtifactError) -> FunctionCallError {
    let message = match error {
        ArtifactError::InvalidReference | ArtifactError::NotFound | ArtifactError::Unauthorized => {
            "exec-output artifact is unavailable in the current thread and workspace".to_string()
        }
        ArtifactError::Incomplete
        | ArtifactError::FinalizationFailed
        | ArtifactError::InvalidState => "exec-output artifact content is incomplete".to_string(),
        ArtifactError::Expired => "exec-output artifact retention has expired".to_string(),
        ArtifactError::Corrupt | ArtifactError::Storage(_) | ArtifactError::Metadata(_) => {
            "exec-output artifact failed integrity validation".to_string()
        }
        ArtifactError::InvalidQuery(message) => {
            format!("exec-output artifact query is invalid: {message}")
        }
        ArtifactError::QuotaExceeded => "exec-output artifact storage quota exceeded".to_string(),
    };
    FunctionCallError::RespondToModel(message)
}

fn record_query_result(
    telemetry: &SessionTelemetry,
    result: &codex_exec_output_artifacts::ArtifactQueryResult,
) {
    let Some(data) = result.data.as_ref() else {
        return;
    };
    let presented_bytes = serde_json::to_vec(data).map_or(0, |data| data.len());
    telemetry.histogram(
        EXEC_OUTPUT_ARTIFACT_PRESENTED_OUTPUT_BYTES_METRIC,
        i64::try_from(presented_bytes).unwrap_or(i64::MAX),
        &[("source", "query")],
    );
    let truncated = match data {
        ArtifactQueryData::Text { truncated, .. }
        | ArtifactQueryData::Bytes { truncated, .. }
        | ArtifactQueryData::Matches { truncated, .. } => *truncated,
    };
    if truncated {
        telemetry.counter(
            EXEC_OUTPUT_ARTIFACT_PREVIEW_TRUNCATION_METRIC,
            /*inc*/ 1,
            &[("source", "query")],
        );
    }
}

fn record_query_error(telemetry: &SessionTelemetry, error: &ArtifactError) {
    match error {
        ArtifactError::Unauthorized => telemetry.counter(
            EXEC_OUTPUT_ARTIFACT_AUTHORIZATION_FAILURE_METRIC,
            /*inc*/ 1,
            &[],
        ),
        ArtifactError::Corrupt | ArtifactError::Storage(_) | ArtifactError::Metadata(_) => {
            telemetry.counter(
                EXEC_OUTPUT_ARTIFACT_CORRUPTION_FAILURE_METRIC,
                /*inc*/ 1,
                &[],
            );
        }
        ArtifactError::InvalidReference
        | ArtifactError::NotFound
        | ArtifactError::Incomplete
        | ArtifactError::FinalizationFailed
        | ArtifactError::InvalidState
        | ArtifactError::Expired
        | ArtifactError::InvalidQuery(_)
        | ArtifactError::QuotaExceeded => {}
    }
}
