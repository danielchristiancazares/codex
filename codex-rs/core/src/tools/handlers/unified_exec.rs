use crate::sandboxing::SandboxPermissions;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell::get_shell_by_model_provided_path;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use crate::unified_exec::ExecCommandMode;
use codex_exec_server::Environment;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_tools::UnifiedExecShellMode;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(test)]
use crate::tools::handlers::parse_arguments;

mod exec_command;
mod write_stdin;

pub use exec_command::ExecCommandHandler;
pub(crate) use exec_command::ExecCommandHandlerOptions;
pub use write_stdin::WriteStdinHandler;

#[derive(Debug, Deserialize)]
pub(crate) struct ExecCommandArgs {
    #[serde(default)]
    pub(crate) cmd: Option<String>,
    #[serde(default)]
    pub(crate) argv: Option<Vec<String>>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default)]
    login: Option<bool>,
    #[serde(default = "default_tty")]
    tty: bool,
    #[serde(default = "default_exec_yield_time_ms")]
    yield_time_ms: u64,
    #[serde(default)]
    max_output_tokens: Option<usize>,
    #[serde(default)]
    sandbox_permissions: Option<SandboxPermissions>,
    #[serde(default)]
    additional_permissions: Option<AdditionalPermissionProfile>,
    #[serde(default)]
    justification: Option<String>,
    #[serde(default)]
    prefix_rule: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ExecCommandEnvironmentArgs {
    #[serde(default)]
    environment_id: Option<String>,
    // Keep this raw until after environment selection; relative paths must be
    // resolved against the selected environment cwd, not the process cwd.
    #[serde(default)]
    workdir: Option<String>,
}

fn default_exec_yield_time_ms() -> u64 {
    10_000
}

fn default_write_stdin_yield_time_ms() -> u64 {
    250
}

fn default_tty() -> bool {
    false
}

#[derive(Debug)]
pub(crate) struct ResolvedCommand {
    pub(crate) command: Vec<String>,
    pub(crate) shell_type: ShellType,
    pub(crate) command_mode: ExecCommandMode,
}

fn post_unified_exec_tool_use_payload(
    invocation: &ToolInvocation,
    result: &dyn ToolOutput,
) -> Option<PostToolUsePayload> {
    let ToolPayload::Function { .. } = &invocation.payload else {
        return None;
    };

    let tool_input = result.post_tool_use_input(&invocation.payload)?;
    let tool_use_id = result.post_tool_use_id(&invocation.call_id);
    let tool_response = result.post_tool_use_response(&tool_use_id, &invocation.payload)?;
    Some(PostToolUsePayload {
        tool_name: HookToolName::bash(),
        tool_use_id,
        tool_input,
        tool_response,
    })
}

pub(crate) fn get_command(
    args: &ExecCommandArgs,
    session_shell: Arc<Shell>,
    shell_mode: &UnifiedExecShellMode,
    allow_login_shell: bool,
) -> Result<ResolvedCommand, String> {
    let command_input = match (&args.cmd, &args.argv) {
        (Some(cmd), None) if !cmd.is_empty() => EitherCommand::Shell(cmd),
        (None, Some(argv)) if !argv.is_empty() && argv.iter().all(|arg| !arg.contains('\0')) => {
            EitherCommand::Argv(argv)
        }
        (Some(_), Some(_)) => return Err("provide exactly one of `cmd` or `argv`".to_string()),
        _ => {
            return Err(
                "`cmd` must be non-empty or `argv` must contain at least one argument".to_string(),
            );
        }
    };
    if matches!(command_input, EitherCommand::Argv(_))
        && (args.shell.is_some() || args.login.is_some())
    {
        return Err("`shell` and `login` are only valid with `cmd`".to_string());
    }
    if let EitherCommand::Argv(argv) = command_input {
        return Ok(ResolvedCommand {
            command: argv.to_vec(),
            shell_type: session_shell.shell_type,
            command_mode: ExecCommandMode::Argv,
        });
    }
    let EitherCommand::Shell(cmd) = command_input else {
        unreachable!("argv returned above");
    };
    let use_login_shell = match args.login {
        Some(true) if !allow_login_shell => {
            return Err(
                "login shell is disabled by config; omit `login` or set it to false.".to_string(),
            );
        }
        Some(use_login_shell) => use_login_shell,
        None => allow_login_shell,
    };

    match shell_mode {
        UnifiedExecShellMode::Direct => {
            let model_shell = args
                .shell
                .as_ref()
                .map(|shell_str| get_shell_by_model_provided_path(&PathBuf::from(shell_str)));
            let shell = model_shell.as_ref().unwrap_or(session_shell.as_ref());
            Ok(ResolvedCommand {
                command: shell.derive_exec_args(cmd, use_login_shell),
                shell_type: shell.shell_type,
                command_mode: ExecCommandMode::Shell,
            })
        }
        UnifiedExecShellMode::ZshFork(zsh_fork_config) => {
            if args.shell.is_some() {
                return Err(
                    "`shell` is not supported for local zsh-fork exec; omit `shell` to use zsh-fork, or target a remote environment where `shell` is supported.".to_string(),
                );
            }

            Ok(ResolvedCommand {
                command: vec![
                    zsh_fork_config.shell_zsh_path.to_string_lossy().to_string(),
                    if use_login_shell { "-lc" } else { "-c" }.to_string(),
                    cmd.to_string(),
                ],
                shell_type: ShellType::Zsh,
                command_mode: ExecCommandMode::Shell,
            })
        }
    }
}

enum EitherCommand<'a> {
    Shell(&'a str),
    Argv(&'a [String]),
}

impl ExecCommandArgs {
    fn hook_command(&self) -> Result<String, String> {
        match (&self.cmd, &self.argv) {
            (Some(cmd), None) if !cmd.is_empty() => Ok(cmd.clone()),
            (None, Some(argv))
                if !argv.is_empty() && argv.iter().all(|arg| !arg.contains('\0')) =>
            {
                Ok(codex_shell_command::parse_command::shlex_join(argv))
            }
            (Some(_), Some(_)) => Err("provide exactly one of `cmd` or `argv`".to_string()),
            _ => Err(
                "`cmd` must be non-empty or `argv` must contain at least one argument".to_string(),
            ),
        }
    }
}

pub(crate) fn shell_mode_for_environment(
    turn_shell_mode: &UnifiedExecShellMode,
    environment: &Environment,
) -> UnifiedExecShellMode {
    if environment.is_remote() {
        UnifiedExecShellMode::Direct
    } else {
        turn_shell_mode.clone()
    }
}

#[cfg(test)]
#[path = "unified_exec_tests.rs"]
mod tests;
