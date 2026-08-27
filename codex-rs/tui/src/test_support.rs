//! Test-only helpers shared across the TUI crate.

use std::sync::LazyLock;

use crate::version::CODEX_CLI_VERSION;
use codex_models_manager::bundled_models_response;
use codex_protocol::openai_models::ModelPreset;
pub(crate) use codex_utils_absolute_path::test_support::PathBufExt;
pub(crate) use codex_utils_absolute_path::test_support::test_path_buf;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) static TEST_MODEL_PRESETS: LazyLock<Vec<ModelPreset>> = LazyLock::new(|| {
    let mut response = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    response.models.sort_by_key(|model| model.priority);
    let mut presets: Vec<ModelPreset> = response.models.into_iter().map(Into::into).collect();
    ModelPreset::mark_default_by_picker_visibility(&mut presets);
    presets
});

pub(crate) fn test_path_display(path: &str) -> String {
    test_path_buf(path).display().to_string()
}

pub(crate) fn sanitize_codex_version(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let Some(version_start) = line.find(CODEX_CLI_VERSION) else {
                return line.to_string();
            };
            let original_width = unicode_width::UnicodeWidthStr::width(line);
            let mut sanitized = line.to_string();
            sanitized.replace_range(
                version_start..version_start + CODEX_CLI_VERSION.len(),
                "0.0.0",
            );
            let sanitized_width = unicode_width::UnicodeWidthStr::width(sanitized.as_str());
            if let Some(border_index) = sanitized.rfind('│')
                && let Some(padding) = original_width.checked_sub(sanitized_width)
            {
                sanitized.insert_str(border_index, &" ".repeat(padding));
            }
            sanitized
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn session_source_cli<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SessionSource::Cli)
}

pub(crate) fn skill_scope_user<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SkillScope::User)
}

pub(crate) fn skill_scope_repo<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SkillScope::Repo)
}

fn from_app_server_wire<T>(value: impl Serialize) -> T
where
    T: DeserializeOwned,
{
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .unwrap_or_else(|err| {
            panic!("app-server wire value should map to legacy helper type: {err}")
        })
}
