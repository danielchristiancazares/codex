use codex_tools::JsonSchema;
use codex_tools::TOOL_SEARCH_MAX_RESULTS;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolSearchSourceInfo;
use codex_tools::ToolSpec;
use codex_utils_string::take_bytes_at_char_boundary;
use std::collections::BTreeMap;

const MAX_TOOL_SEARCH_SOURCE_DESCRIPTION_BYTES: usize = 16 * 1024;
const TOOL_SEARCH_SOURCE_TRUNCATION_MARKER: &str = "\n… source listing truncated …";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolSearchSourceListing {
    Include,
    Omit,
}

pub(crate) fn create_tool_search_tool(
    searchable_sources: &[ToolSearchSourceInfo],
    default_limit: usize,
    source_listing: ToolSearchSourceListing,
) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "query".to_string(),
            JsonSchema::string(Some("Search query for deferred tools.".to_string())),
        ),
        (
            "limit".to_string(),
            JsonSchema::integer(Some(format!(
                "Maximum number of tools to return. Defaults to {default_limit}."
            )))
            .with_integer_bounds(
                /*minimum*/ 1,
                /*maximum*/ TOOL_SEARCH_MAX_RESULTS as i64,
            ),
        ),
    ]);

    let source_section = match source_listing {
        ToolSearchSourceListing::Include => {
            let mut source_descriptions = BTreeMap::new();
            for source in searchable_sources {
                source_descriptions
                    .entry(source.name.clone())
                    .and_modify(|existing: &mut Option<String>| {
                        if existing.is_none() {
                            *existing = source.description.clone();
                        }
                    })
                    .or_insert(source.description.clone());
            }

            let source_descriptions = if source_descriptions.is_empty() {
                "None currently enabled.".to_string()
            } else {
                let full_len =
                    source_descriptions.iter().enumerate().fold(
                        0usize,
                        |total, (index, (name, description))| {
                            total
                                .saturating_add(usize::from(index > 0))
                                .saturating_add(2)
                                .saturating_add(name.len())
                                .saturating_add(description.as_ref().map_or(0, |description| {
                                    2usize.saturating_add(description.len())
                                }))
                        },
                    );
                let was_truncated = full_len > MAX_TOOL_SEARCH_SOURCE_DESCRIPTION_BYTES;
                let marker_len = if was_truncated {
                    TOOL_SEARCH_SOURCE_TRUNCATION_MARKER.len()
                } else {
                    0
                };
                let content_limit =
                    MAX_TOOL_SEARCH_SOURCE_DESCRIPTION_BYTES.saturating_sub(marker_len);
                let reserved_name_bytes = source_descriptions.keys().fold(
                    source_descriptions.len().saturating_sub(1),
                    |reserved, name| reserved.saturating_add(2).saturating_add(name.len()),
                );
                let mut description_budget = content_limit.saturating_sub(reserved_name_bytes);
                let mut rendered = String::new();
                for (name, description) in source_descriptions {
                    let separator_bytes = usize::from(!rendered.is_empty());
                    let required = separator_bytes.saturating_add(2).saturating_add(name.len());
                    if required > content_limit.saturating_sub(rendered.len()) {
                        break;
                    }

                    if !rendered.is_empty() {
                        rendered.push('\n');
                    }
                    rendered.push_str("- ");
                    rendered.push_str(&name);

                    if let Some(description) = description
                        && description_budget >= 2
                    {
                        rendered.push_str(": ");
                        description_budget -= 2;
                        let bounded_description =
                            take_bytes_at_char_boundary(&description, description_budget);
                        rendered.push_str(bounded_description);
                        description_budget -= bounded_description.len();
                    }
                }
                if was_truncated {
                    rendered.push_str(TOOL_SEARCH_SOURCE_TRUNCATION_MARKER);
                }
                rendered
            };
            format!(
                "\n\nYou have access to tools from the following sources:\n{source_descriptions}\n"
            )
        }
        ToolSearchSourceListing::Omit => "\n\n".to_string(),
    };

    let description = format!(
        "# Tool discovery\n\nSearches over deferred tool metadata with BM25 and exposes matching tools for the next model call.{source_section}Some of the tools may not have been provided to you upfront, and you should use this tool (`{TOOL_SEARCH_TOOL_NAME}`) to search for the required tools. For MCP tool discovery, always use `{TOOL_SEARCH_TOOL_NAME}` instead of `list_mcp_resources` or `list_mcp_resource_templates`."
    );

    ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["query".to_string()]),
            Some(false.into()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::JsonSchema;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    #[test]
    fn create_tool_search_tool_deduplicates_and_renders_enabled_sources() {
        assert_eq!(
            create_tool_search_tool(
                &[
                    ToolSearchSourceInfo {
                        name: "Google Drive".to_string(),
                        description: Some(
                            "Use Google Drive as the single entrypoint for Drive, Docs, Sheets, and Slides work."
                                .to_string(),
                        ),
                    },
                    ToolSearchSourceInfo {
                        name: "Google Drive".to_string(),
                        description: None,
                    },
                    ToolSearchSourceInfo {
                        name: "docs".to_string(),
                        description: None,
                    },
                ],
                /*default_limit*/ 8,
                ToolSearchSourceListing::Include,
            ),
            ToolSpec::ToolSearch {
                execution: "client".to_string(),
                description: "# Tool discovery\n\nSearches over deferred tool metadata with BM25 and exposes matching tools for the next model call.\n\nYou have access to tools from the following sources:\n- Google Drive: Use Google Drive as the single entrypoint for Drive, Docs, Sheets, and Slides work.\n- docs\nSome of the tools may not have been provided to you upfront, and you should use this tool (`tool_search`) to search for the required tools. For MCP tool discovery, always use `tool_search` instead of `list_mcp_resources` or `list_mcp_resource_templates`.".to_string(),
                parameters: JsonSchema::object(BTreeMap::from([
                        (
                            "limit".to_string(),
                            JsonSchema::integer(Some(
                                    "Maximum number of tools to return. Defaults to 8."
                                        .to_string(),
                                ),)
                                .with_integer_bounds(/*minimum*/ 1, /*maximum*/ 32),
                        ),
                        (
                            "query".to_string(),
                            JsonSchema::string(Some("Search query for deferred tools.".to_string()),),
                        ),
                    ]), Some(vec!["query".to_string()]), Some(false.into())),
            }
        );
    }

    #[test]
    fn create_tool_search_tool_omits_sources_when_world_state_advertises_them() {
        let ToolSpec::ToolSearch { description, .. } = create_tool_search_tool(
            &[ToolSearchSourceInfo {
                name: "Google Drive".to_string(),
                description: Some("Search files and documents.".to_string()),
            }],
            /*default_limit*/ 8,
            ToolSearchSourceListing::Omit,
        ) else {
            panic!("expected tool search spec");
        };

        assert!(!description.contains("You have access to tools from the following sources"));
        assert!(!description.contains("Google Drive"));
        assert!(description.contains("use this tool (`tool_search`) to search"));
    }

    #[test]
    fn create_tool_search_tool_bounds_aggregate_source_descriptions() {
        let long_description = "🦀".repeat(20_000);
        let sources = (0..8)
            .map(|index| ToolSearchSourceInfo {
                name: format!("source-{index:02}"),
                description: Some(long_description.clone()),
            })
            .collect::<Vec<_>>();
        let ToolSpec::ToolSearch { description, .. } = create_tool_search_tool(
            &sources,
            /*default_limit*/ 8,
            ToolSearchSourceListing::Include,
        ) else {
            panic!("expected tool search spec");
        };

        let (_, source_section) = description
            .split_once("You have access to tools from the following sources:\n")
            .expect("tool search should retain its source introduction");
        let (source_descriptions, _) = source_section
            .split_once("\nSome of the tools may not have been provided to you upfront")
            .expect("tool search should retain its discovery instructions");
        assert!(source_descriptions.len() <= MAX_TOOL_SEARCH_SOURCE_DESCRIPTION_BYTES);
        assert!(source_descriptions.starts_with("- source-00: 🦀"));
        assert!(source_descriptions.contains("… source listing truncated …"));
        let advertised_names = source_descriptions
            .lines()
            .filter(|line| line.starts_with("- "))
            .map(|line| {
                let source = line
                    .strip_prefix("- ")
                    .expect("each source should be a complete list item");
                source
                    .split_once(": ")
                    .map_or(source, |(name, _)| name)
                    .to_string()
            })
            .collect::<Vec<_>>();
        let expected_names = (0..8)
            .map(|index| format!("source-{index:02}"))
            .collect::<Vec<_>>();
        assert_eq!(advertised_names, expected_names);
        assert!(description.contains("always use `tool_search`"));
    }
}
