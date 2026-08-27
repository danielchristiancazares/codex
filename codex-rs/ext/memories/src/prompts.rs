use crate::MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use codex_utils_template::Template;
use std::sync::LazyLock;
use tokio::fs;
use tokio::io::AsyncReadExt;

const MEMORY_SUMMARY_MAX_FILE_BYTES: u64 = 64 * 1024;

static MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    parse_embedded_template(
        include_str!("../templates/memories/read_path.md"),
        "memories/read_path.md",
    )
});

fn parse_embedded_template(source: &'static str, template_name: &str) -> Template {
    match Template::parse(source) {
        Ok(template) => template,
        Err(err) => panic!("embedded template {template_name} is invalid: {err}"),
    }
}

/// Build the memory read-path prompt that is added to developer instructions.
///
/// Files above [MEMORY_SUMMARY_MAX_FILE_BYTES] are rejected. Accepted summaries are truncated at
/// [MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT].
pub(crate) async fn build_memory_tool_developer_instructions(
    codex_home: &AbsolutePathBuf,
) -> Option<String> {
    let base_path = codex_home.join("memories");
    let memory_summary_path = base_path.join("memory_summary.md");
    let file = fs::File::open(&memory_summary_path).await.ok()?;
    let file_len = file.metadata().await.ok()?.len();
    if file_len > MEMORY_SUMMARY_MAX_FILE_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(usize::try_from(file_len).ok()?);
    file.take(/*limit*/ MEMORY_SUMMARY_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() as u64 > MEMORY_SUMMARY_MAX_FILE_BYTES {
        return None;
    }
    let memory_summary = String::from_utf8(bytes).ok()?.trim().to_string();
    let memory_summary = truncate_text(
        &memory_summary,
        TruncationPolicy::Tokens(MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT),
    );
    if memory_summary.is_empty() {
        return None;
    }
    let base_path = base_path.display().to_string();
    MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_TEMPLATE
        .render([
            ("base_path", base_path.as_str()),
            ("memory_summary", memory_summary.as_str()),
        ])
        .ok()
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
