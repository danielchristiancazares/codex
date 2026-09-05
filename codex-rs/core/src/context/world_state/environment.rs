use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::environment_context::FileSystemContext;
use crate::context::environment_context::NetworkContext;
use crate::context::environment_context::push_xml_escaped_text;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::turn_context::TurnContext;
use crate::shell::ShellType;
use codex_features::Feature;
use codex_protocol::models::ContentItemKind;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Mutex;

static POWERSHELL_VERSIONS: LazyLock<Mutex<BTreeMap<PathBuf, Option<String>>>> =
    LazyLock::new(Mutex::default);

/// Environment values visible to the model.
#[derive(Clone, Debug, Default)]
pub(crate) struct EnvironmentsState {
    environments: BTreeMap<String, EnvironmentState>,
    shell_version: Option<String>,
    current_date: Option<String>,
    timezone: Option<String>,
    network: Option<NetworkContext>,
    filesystem: Option<FileSystemContext>,
    subagents: Option<String>,
}

impl EnvironmentsState {
    pub(crate) async fn from_turn_context_with_environments(
        turn_context: &TurnContext,
        environments: &TurnEnvironmentSnapshot,
        current_date: Option<String>,
    ) -> Self {
        let shell_version = if turn_context
            .config
            .features
            .enabled(Feature::PowerShellShellVersion)
            && let Some(environment) = environments.single_local_environment()
            && let Some(shell) = environment.shell.as_ref()
            && shell.shell_type == ShellType::PowerShell
        {
            powershell_version(&shell.shell_path).await
        } else {
            None
        };
        Self {
            environments: environment_states(environments),
            shell_version,
            current_date,
            timezone: turn_context.timezone.clone(),
            network: network_from_turn_context(turn_context),
            filesystem: environments.primary().map(|environment| {
                FileSystemContext::from_permission_profile(
                    environment.permission_profile(),
                    environment.workspace_roots(),
                )
            }),
            subagents: None,
        }
    }

    pub(crate) fn with_subagents(mut self, subagents: String) -> Self {
        if !subagents.is_empty() {
            self.subagents = Some(subagents);
        }
        self
    }

    fn rendered_full(&self) -> RenderedEnvironments {
        RenderedEnvironments {
            updates: self
                .environments
                .iter()
                .map(|(id, environment)| {
                    (id.clone(), EnvironmentUpdate::Current(environment.clone()))
                })
                .collect(),
            legacy_single: is_legacy_single(&self.environments),
            include_primary: self.environments.len() > 1,
            shell_version: self
                .shell_version
                .clone()
                .map_or(ShellVersionUpdate::Unchanged, ShellVersionUpdate::Current),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            network: self.network.clone(),
            filesystem: self.filesystem.clone(),
            subagents: self.subagents.clone(),
        }
    }
}

impl WorldStateSection for EnvironmentsState {
    const ID: &'static str = "environments";
    type Snapshot = EnvironmentsSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        EnvironmentsSnapshot {
            environments: self
                .environments
                .iter()
                .map(|(id, environment)| {
                    (
                        id.clone(),
                        EnvironmentSnapshot {
                            cwd: environment.cwd.inferred_native_path_string(),
                            status: environment.status,
                            shell: environment.shell.clone(),
                            is_primary: self.environments.len() > 1 && environment.is_primary,
                        },
                    )
                })
                .collect(),
            shell_version: self.shell_version.clone(),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            network: self.network.as_ref().map(NetworkContext::render),
            filesystem: self.filesystem.as_ref().map(FileSystemContext::render),
            subagents: self.subagents.clone(),
        }
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        let current = self.snapshot();
        let empty = EnvironmentsSnapshot::default();
        let previous = match previous {
            PreviousSectionState::Known(previous) => previous,
            PreviousSectionState::Absent | PreviousSectionState::Unknown => &empty,
        };
        let shell_version_changed = current.shell_version != previous.shell_version;
        let current_date_changed = current.current_date != previous.current_date;
        let timezone_changed = current.timezone != previous.timezone;
        let network_changed = current.network != previous.network;
        let filesystem_changed = current.filesystem != previous.filesystem;
        let subagents_changed = current.subagents != previous.subagents;
        let turn_context_values_changed = shell_version_changed
            || current_date_changed
            || timezone_changed
            || network_changed
            || filesystem_changed
            || subagents_changed;
        let multiple_environments = self.environments.len() > 1;
        let previous_multiple_environments = previous.environments.len() > 1;
        let mut updates = self
            .environments
            .iter()
            .filter_map(|(id, environment)| {
                let update = match previous.environments.get(id) {
                    None => EnvironmentUpdate::Current(environment.clone()),
                    Some(_) if multiple_environments != previous_multiple_environments => {
                        EnvironmentUpdate::Current(environment.clone())
                    }
                    Some(previous) => {
                        let delta = EnvironmentDelta::between(
                            environment,
                            &current.environments[id],
                            previous,
                        );
                        if delta.is_empty() {
                            return None;
                        }
                        EnvironmentUpdate::Changed(delta)
                    }
                };
                Some((id.clone(), update))
            })
            .collect::<BTreeMap<_, _>>();
        updates.extend(
            previous
                .environments
                .keys()
                .filter(|id| !self.environments.contains_key(*id))
                .map(|id| (id.clone(), EnvironmentUpdate::Unavailable)),
        );
        let legacy_single = is_legacy_single(&self.environments)
            && updates
                .values()
                .all(|update| !matches!(update, EnvironmentUpdate::Unavailable));
        (!updates.is_empty() || turn_context_values_changed).then(|| {
            Box::new(RenderedEnvironments {
                updates,
                legacy_single,
                include_primary: multiple_environments || previous_multiple_environments,
                shell_version: if shell_version_changed {
                    self.shell_version
                        .clone()
                        .map_or(ShellVersionUpdate::Unavailable, ShellVersionUpdate::Current)
                } else {
                    ShellVersionUpdate::Unchanged
                },
                current_date: self.current_date.clone(),
                timezone: self.timezone.clone(),
                network: network_changed.then(|| self.network.clone()).flatten(),
                filesystem: filesystem_changed
                    .then(|| self.filesystem.clone())
                    .flatten(),
                subagents: subagents_changed.then(|| self.subagents.clone()).flatten(),
            }) as Box<dyn ContextualUserFragment>
        })
    }
}

impl ContextualUserFragment for EnvironmentsState {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("environments.environment_context".to_string())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        environment_context_markers()
    }

    fn body(&self) -> String {
        self.rendered_full().body()
    }
}

struct RenderedEnvironments {
    updates: BTreeMap<String, EnvironmentUpdate>,
    legacy_single: bool,
    include_primary: bool,
    shell_version: ShellVersionUpdate,
    current_date: Option<String>,
    timezone: Option<String>,
    network: Option<NetworkContext>,
    filesystem: Option<FileSystemContext>,
    subagents: Option<String>,
}

enum ShellVersionUpdate {
    Unchanged,
    Current(String),
    Unavailable,
}

enum EnvironmentUpdate {
    Current(EnvironmentState),
    Changed(EnvironmentDelta),
    Unavailable,
}

struct EnvironmentDelta {
    cwd: Option<PathUri>,
    status: Option<EnvironmentStatus>,
    shell: Option<Option<String>>,
    is_primary: Option<bool>,
}

impl EnvironmentDelta {
    fn between(
        current: &EnvironmentState,
        current_snapshot: &EnvironmentSnapshot,
        previous: &EnvironmentSnapshot,
    ) -> Self {
        Self {
            cwd: (current_snapshot.cwd != previous.cwd).then(|| current.cwd.clone()),
            status: (current_snapshot.status != previous.status).then_some(current.status),
            shell: (current_snapshot.shell != previous.shell).then(|| current.shell.clone()),
            is_primary: (current_snapshot.is_primary != previous.is_primary)
                .then_some(current_snapshot.is_primary),
        }
    }

    fn is_empty(&self) -> bool {
        self.cwd.is_none()
            && self.status.is_none()
            && self.shell.is_none()
            && self.is_primary.is_none()
    }
}

impl ContextualUserFragment for RenderedEnvironments {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("environments.environment_context".to_string())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        environment_context_markers()
    }

    fn body(&self) -> String {
        let mut rendered = "\n".to_string();
        if self.legacy_single {
            if let Some(update) = self.updates.values().next() {
                match update {
                    EnvironmentUpdate::Current(environment) => {
                        push_environment_values(&mut rendered, environment, "  ");
                    }
                    EnvironmentUpdate::Changed(delta) => {
                        push_environment_delta(&mut rendered, delta, "  ");
                    }
                    EnvironmentUpdate::Unavailable => {}
                }
            }
        } else if !self.updates.is_empty() {
            rendered.push_str("  <environments>\n");
            for (id, update) in &self.updates {
                match update {
                    EnvironmentUpdate::Current(environment) => {
                        rendered.push_str("    <environment id=\"");
                        push_xml_escaped_text(&mut rendered, id);
                        rendered.push('"');
                        if self.include_primary {
                            rendered.push_str(if environment.is_primary {
                                " primary=\"true\""
                            } else {
                                " primary=\"false\""
                            });
                        }
                        rendered.push_str(">\n");
                        push_environment_values(&mut rendered, environment, "      ");
                        rendered.push_str("    </environment>\n");
                    }
                    EnvironmentUpdate::Changed(delta) => {
                        rendered.push_str("    <environment id=\"");
                        push_xml_escaped_text(&mut rendered, id);
                        rendered.push('"');
                        if self.include_primary
                            && let Some(is_primary) = delta.is_primary
                        {
                            rendered.push_str(if is_primary {
                                " primary=\"true\""
                            } else {
                                " primary=\"false\""
                            });
                        }
                        rendered.push_str(">\n");
                        push_environment_delta(&mut rendered, delta, "      ");
                        rendered.push_str("    </environment>\n");
                    }
                    EnvironmentUpdate::Unavailable => {
                        rendered.push_str("    <environment id=\"");
                        push_xml_escaped_text(&mut rendered, id);
                        rendered.push_str("\" status=\"unavailable\" />\n");
                    }
                }
            }
            rendered.push_str("  </environments>\n");
        }
        match &self.shell_version {
            ShellVersionUpdate::Unchanged => {}
            ShellVersionUpdate::Current(version) => {
                push_optional_element(&mut rendered, "shell_version", Some(version));
            }
            ShellVersionUpdate::Unavailable => {
                rendered.push_str("  <shell_version status=\"unavailable\" />\n");
            }
        }
        push_optional_element(&mut rendered, "current_date", self.current_date.as_deref());
        push_optional_element(&mut rendered, "timezone", self.timezone.as_deref());
        if let Some(network) = &self.network {
            rendered.push_str("  ");
            rendered.push_str(&network.render());
            rendered.push('\n');
        }
        if let Some(filesystem) = &self.filesystem {
            rendered.push_str("  ");
            rendered.push_str(&filesystem.render());
            rendered.push('\n');
        }
        if let Some(subagents) = &self.subagents {
            rendered.push_str("  <subagents>\n");
            for line in subagents.lines() {
                rendered.push_str("    ");
                rendered.push_str(line);
                rendered.push('\n');
            }
            rendered.push_str("  </subagents>\n");
        }
        rendered
    }
}

fn push_environment_values(rendered: &mut String, environment: &EnvironmentState, indent: &str) {
    rendered.push_str(indent);
    rendered.push_str("<cwd>");
    push_xml_escaped_text(rendered, &environment.cwd.inferred_native_path_string());
    rendered.push_str("</cwd>\n");
    if environment.status == EnvironmentStatus::Starting {
        rendered.push_str(indent);
        rendered.push_str("<status>starting</status>\n");
    }
    if let Some(shell) = &environment.shell {
        rendered.push_str(indent);
        rendered.push_str("<shell>");
        push_xml_escaped_text(rendered, shell);
        rendered.push_str("</shell>\n");
    }
}

fn push_environment_delta(rendered: &mut String, delta: &EnvironmentDelta, indent: &str) {
    if let Some(cwd) = &delta.cwd {
        rendered.push_str(indent);
        rendered.push_str("<cwd>");
        push_xml_escaped_text(rendered, &cwd.inferred_native_path_string());
        rendered.push_str("</cwd>\n");
    }
    if let Some(status) = delta.status {
        rendered.push_str(indent);
        rendered.push_str("<status>");
        rendered.push_str(match status {
            EnvironmentStatus::Starting => "starting",
            EnvironmentStatus::Available => "available",
        });
        rendered.push_str("</status>\n");
    }
    if let Some(shell) = &delta.shell {
        rendered.push_str(indent);
        if let Some(shell) = shell {
            rendered.push_str("<shell>");
            push_xml_escaped_text(rendered, shell);
            rendered.push_str("</shell>\n");
        } else {
            rendered.push_str("<shell status=\"unavailable\" />\n");
        }
    }
}

fn push_optional_element(rendered: &mut String, name: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    rendered.push_str("  <");
    rendered.push_str(name);
    rendered.push('>');
    push_xml_escaped_text(rendered, value);
    rendered.push_str("</");
    rendered.push_str(name);
    rendered.push_str(">\n");
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EnvironmentState {
    cwd: PathUri,
    status: EnvironmentStatus,
    shell: Option<String>,
    is_primary: bool,
}

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct EnvironmentsSnapshot {
    environments: BTreeMap<String, EnvironmentSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shell_version: Option<String>,
    current_date: Option<String>,
    timezone: Option<String>,
    network: Option<String>,
    filesystem: Option<String>,
    subagents: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct EnvironmentSnapshot {
    cwd: String,
    status: EnvironmentStatus,
    shell: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    is_primary: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EnvironmentStatus {
    Starting,
    Available,
}

async fn powershell_version(shell_path: &Path) -> Option<String> {
    if let Some(version) = {
        let versions = POWERSHELL_VERSIONS.lock().await;
        versions.get(shell_path).cloned()
    } {
        return version;
    }

    let mut command = Command::new(shell_path);
    command
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$PSVersionTable.PSVersion.ToString()",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    let version = tokio::time::timeout(Duration::from_secs(2), command.output())
        .await
        .ok()
        .and_then(Result::ok)
        .filter(|output| output.status.success() && output.stdout.len() <= 64)
        .and_then(|output| {
            let mut components = std::str::from_utf8(&output.stdout).ok()?.trim().split('.');
            let major = components.next()?.parse::<u16>().ok()?;
            let minor = components.next()?.parse::<u16>().ok()?;
            Some(format!("{major}.{minor}"))
        });
    POWERSHELL_VERSIONS
        .lock()
        .await
        .insert(shell_path.to_owned(), version.clone());
    version
}

fn environment_states(snapshot: &TurnEnvironmentSnapshot) -> BTreeMap<String, EnvironmentState> {
    let mut environments = snapshot
        .turn_environments()
        .enumerate()
        .map(|(index, environment)| {
            (
                environment.selection.environment_id.clone(),
                EnvironmentState {
                    cwd: environment.cwd().clone(),
                    status: EnvironmentStatus::Available,
                    shell: environment
                        .shell
                        .as_ref()
                        .map(|shell| shell.name().to_string()),
                    is_primary: index == 0,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    for environment in snapshot.starting() {
        environments
            .entry(environment.selection.environment_id.clone())
            .or_insert_with(|| EnvironmentState {
                cwd: environment.selection.cwd.clone(),
                status: EnvironmentStatus::Starting,
                shell: None,
                is_primary: false,
            });
    }
    environments
}

fn is_legacy_single(environments: &BTreeMap<String, EnvironmentState>) -> bool {
    environments.len() == 1
        && environments
            .values()
            .all(|environment| environment.status == EnvironmentStatus::Available)
}

fn environment_context_markers() -> (&'static str, &'static str) {
    (
        codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG,
        codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG,
    )
}

fn network_from_turn_context(turn_context: &TurnContext) -> Option<NetworkContext> {
    let network = turn_context
        .config
        .config_layer_stack
        .requirements()
        .network
        .as_ref()?;

    Some(NetworkContext::new(
        network
            .domains
            .as_ref()
            .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
            .unwrap_or_default(),
        network
            .domains
            .as_ref()
            .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains)
            .unwrap_or_default(),
    ))
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "environment_render_tests.rs"]
mod render_tests;
