use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use serde_json::json;
use std::collections::BTreeMap;

use codex_exec_output_artifacts::DEFAULT_QUERY_BYTES_CAP;

pub(crate) const EXEC_OUTPUT_QUERY_TOOL_NAME: &str = "exec_output_query";

pub(crate) fn create_exec_output_query_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "artifact_ref".to_string(),
            JsonSchema::string(Some(
                "Opaque exec-output artifact reference returned by `exec_command`.".to_string(),
            )),
        ),
        (
            "view".to_string(),
            JsonSchema::string_enum(
                vec![
                    json!("metadata"),
                    json!("head"),
                    json!("tail"),
                    json!("bytes"),
                    json!("lines"),
                    json!("search"),
                ],
                Some(
                    "Bounded view to read. `tail` is available when capture is complete."
                        .to_string(),
                ),
            ),
        ),
        (
            "environment_id".to_string(),
            JsonSchema::string(Some(
                "Owning environment id. Omit to use the primary environment.".to_string(),
            )),
        ),
        (
            "max_output_bytes".to_string(),
            JsonSchema::number(Some(format!(
                "Maximum returned content bytes. Defaults to 8192 and is capped at {DEFAULT_QUERY_BYTES_CAP}."
            ))),
        ),
        (
            "start".to_string(),
            JsonSchema::number(Some(
                "Zero-based byte offset for `bytes`, or one-based first line for `lines`."
                    .to_string(),
            )),
        ),
        (
            "end".to_string(),
            JsonSchema::number(Some(
                "One-based inclusive final line for `lines`.".to_string(),
            )),
        ),
        (
            "length".to_string(),
            JsonSchema::number(Some(
                "Requested byte length for the `bytes` view.".to_string(),
            )),
        ),
        (
            "pattern".to_string(),
            JsonSchema::string(Some(
                "Literal or regular-expression pattern for `search`.".to_string(),
            )),
        ),
        (
            "search_mode".to_string(),
            JsonSchema::string_enum(
                vec![json!("literal"), json!("regex")],
                Some("Pattern interpretation for `search`; defaults to literal.".to_string()),
            ),
        ),
        (
            "case_sensitive".to_string(),
            JsonSchema::boolean(Some(
                "Use case-sensitive matching for `search`; defaults to true.".to_string(),
            )),
        ),
        (
            "context_lines".to_string(),
            JsonSchema::number(Some(
                "Surrounding lines per search match; defaults to 0 and caps at 10.".to_string(),
            )),
        ),
        (
            "max_matches".to_string(),
            JsonSchema::number(Some(
                "Maximum search matches; defaults to 20 and caps at 100.".to_string(),
            )),
        ),
        (
            "include_data".to_string(),
            JsonSchema::boolean(Some(
                "Return slice data even when the same slice was already presented in this context epoch; defaults to false.".to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: EXEC_OUTPUT_QUERY_TOOL_NAME.to_string(),
        description: "Reads a bounded view of stdout or stderr retained by `exec_command`. Access stays scoped to the owning thread, environment, and workspace roots.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["artifact_ref".to_string(), "view".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}
