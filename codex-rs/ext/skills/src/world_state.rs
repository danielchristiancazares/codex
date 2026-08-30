use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use crate::render::SkillRenderReport;

pub(crate) const SKILLS_WORLD_STATE_ID: &str = "skills";
pub(crate) const ORCHESTRATOR_SKILLS_WORLD_STATE_ID: &str = "orchestrator_skills";
pub(crate) const HOST_SKILLS_WORLD_STATE_ID: &str = "host_skills";
const NO_EXECUTOR_SKILLS_BODY: &str =
    "\n## Skills update\nNo selected-environment skills are currently available.\n";
const HIDDEN_EXECUTOR_SKILLS_BODY: &str = "\n## Skills update\nSelected-environment skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_ORCHESTRATOR_SKILLS_BODY: &str =
    "\n## Orchestrator skills update\nNo orchestrator skills are currently available.\n";
const HIDDEN_ORCHESTRATOR_SKILLS_BODY: &str = "\n## Orchestrator skills update\nOrchestrator skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const NO_HOST_SKILLS_BODY: &str =
    "\n## Host skills update\nNo host skills are currently available.\n";
const HIDDEN_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are not listed automatically. Explicit skill mentions can still be resolved when available.\n";
const OMITTED_HOST_SKILLS_BODY: &str = "\n## Host skills update\nHost skills are available but omitted from the model-visible skills list because the skills context budget was exceeded.\n";

pub(crate) type CatalogRenderCallback = Box<dyn Fn() + Send + Sync>;

#[derive(Clone, Default)]
pub(crate) struct CatalogLines {
    pub(crate) roots: Vec<String>,
    pub(crate) skills: Vec<String>,
}

pub(crate) fn executor_skills_world_state_section(
    body: Option<String>,
    lines: CatalogLines,
    include_instructions: bool,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    skills_world_state_section(
        SKILLS_WORLD_STATE_ID,
        body,
        lines,
        include_instructions,
        /*enabled*/ None,
        NO_EXECUTOR_SKILLS_BODY,
        HIDDEN_EXECUTOR_SKILLS_BODY,
        on_render,
    )
    .with_legacy_matcher(|role, text| {
        role == "developer"
            && text.trim_start().starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
            && text.trim_end().ends_with(SKILLS_INSTRUCTIONS_CLOSE_TAG)
    })
}

pub(crate) fn orchestrator_skills_world_state_section(
    body: Option<String>,
    lines: CatalogLines,
    include_instructions: bool,
    enabled: bool,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    skills_world_state_section(
        ORCHESTRATOR_SKILLS_WORLD_STATE_ID,
        body,
        lines,
        include_instructions,
        Some(enabled),
        NO_ORCHESTRATOR_SKILLS_BODY,
        if enabled {
            HIDDEN_ORCHESTRATOR_SKILLS_BODY
        } else {
            NO_ORCHESTRATOR_SKILLS_BODY
        },
        on_render,
    )
}

fn skills_world_state_section(
    id: &'static str,
    body: Option<String>,
    lines: CatalogLines,
    include_instructions: bool,
    enabled: Option<bool>,
    no_skills_body: &'static str,
    hidden_skills_body: &'static str,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    let mut snapshot = json!({
        "body": body,
        "rootLines": lines.roots,
        "skillLines": lines.skills,
        "includeInstructions": include_instructions,
    });
    if let Some(enabled) = enabled {
        snapshot["enabled"] = json!(enabled);
    }
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(&snapshot)
        .expect("skill catalog snapshot should serialize")
        .hash(&mut hasher);
    let revision = format!("{:016x}", hasher.finish());
    snapshot["revision"] = json!(revision);
    let retained_revision = body
        .as_ref()
        .map(|_| format!("<!-- skills-catalog-revision:{revision} -->"));

    let contribution = WorldStateSectionContribution::new(id, snapshot, move |previous| {
        if let PreviousWorldStateSection::Known(previous) = &previous {
            if previous.get("revision").and_then(serde_json::Value::as_str)
                == Some(revision.as_str())
            {
                return None;
            }
        }

        let mut rendered_body = match body.as_deref() {
            Some(body) => {
                let delta = match &previous {
                    PreviousWorldStateSection::Known(previous)
                        if previous
                            .get("includeInstructions")
                            .and_then(serde_json::Value::as_bool)
                            == Some(include_instructions)
                            && previous.get("enabled").and_then(serde_json::Value::as_bool)
                                == enabled
                            && previous
                                .get("body")
                                .and_then(serde_json::Value::as_str)
                                .is_some() =>
                    {
                        let previous_roots = previous
                            .get("rootLines")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|lines| {
                                lines
                                    .iter()
                                    .map(serde_json::Value::as_str)
                                    .collect::<Option<Vec<_>>>()
                            });
                        let previous_skills = previous
                            .get("skillLines")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|lines| {
                                lines
                                    .iter()
                                    .map(serde_json::Value::as_str)
                                    .collect::<Option<Vec<_>>>()
                            });
                        previous_roots.zip(previous_skills).and_then(
                            |(previous_roots, previous_skills)| {
                                render_catalog_delta(&lines, &previous_roots, &previous_skills)
                            },
                        )
                    }
                    PreviousWorldStateSection::Absent
                    | PreviousWorldStateSection::Unknown
                    | PreviousWorldStateSection::Known(_) => None,
                };
                delta.unwrap_or_else(|| body.to_string())
            }
            None if matches!(previous, PreviousWorldStateSection::Absent) => return None,
            None if !include_instructions => hidden_skills_body.to_string(),
            None => no_skills_body.to_string(),
        };
        if body.is_some() {
            if !rendered_body.ends_with('\n') {
                rendered_body.push('\n');
            }
            rendered_body.push_str("<!-- skills-catalog-revision:");
            rendered_body.push_str(&revision);
            rendered_body.push_str(" -->\n");
        }
        on_render();

        Some(RenderedWorldStateFragment::new(
            "developer",
            (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG),
            rendered_body,
        ))
    });
    match retained_revision {
        Some(revision) => contribution.with_retained_fragment_matcher(move |role, text| {
            role == "developer" && text.contains(&revision)
        }),
        None => contribution,
    }
}

fn render_catalog_delta(
    current: &CatalogLines,
    previous_roots: &[&str],
    previous_skills: &[&str],
) -> Option<String> {
    let added_roots = current
        .roots
        .iter()
        .filter(|line| !previous_roots.contains(&line.as_str()))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let removed_roots = previous_roots
        .iter()
        .filter(|line| {
            !current
                .roots
                .iter()
                .any(|current| current.as_str() == **line)
        })
        .copied()
        .collect::<Vec<_>>();
    let added_skills = current
        .skills
        .iter()
        .filter(|line| !previous_skills.contains(&line.as_str()))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let removed_skills = previous_skills
        .iter()
        .filter(|line| {
            !current
                .skills
                .iter()
                .any(|current| current.as_str() == **line)
        })
        .copied()
        .collect::<Vec<_>>();
    if added_roots.is_empty()
        && removed_roots.is_empty()
        && added_skills.is_empty()
        && removed_skills.is_empty()
    {
        return None;
    }

    let mut body = "\n## Skills update\n".to_string();
    for (heading, lines) in [
        ("### Added or updated skill roots", added_roots),
        ("### Removed skill roots", removed_roots),
        ("### Added or updated skills", added_skills),
        ("### Removed skills", removed_skills),
    ] {
        if lines.is_empty() {
            continue;
        }
        body.push_str(heading);
        body.push('\n');
        for line in lines {
            body.push_str(line);
            body.push('\n');
        }
    }
    Some(body)
}

pub(crate) fn host_skills_world_state_section(
    body: Option<String>,
    lines: CatalogLines,
    include_instructions: bool,
    report: &SkillRenderReport,
    on_render: CatalogRenderCallback,
) -> WorldStateSectionContribution {
    let body = body.or_else(|| {
        (report.included_count == 0 && report.omitted_count > 0)
            .then(|| OMITTED_HOST_SKILLS_BODY.to_string())
    });
    skills_world_state_section(
        HOST_SKILLS_WORLD_STATE_ID,
        body,
        lines,
        include_instructions,
        /*enabled*/ None,
        NO_HOST_SKILLS_BODY,
        HIDDEN_HOST_SKILLS_BODY,
        on_render,
    )
}

#[cfg(test)]
#[path = "world_state_tests.rs"]
mod tests;
