use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use super::CommandHookRuntime;
use super::CommandShell;
use super::ConfiguredHandler;
use super::super::ConfiguredHandlerKind;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

const PARENT: &str = "CODEX_HOOK_NO_CONSOLE_PARENT";
const PROBE: &str = "engine::command_runner::windows_console_tests::console_probe";

#[test]
#[ignore = "child executable used by console regression tests"]
fn console_probe() {
    // SAFETY: Read-only query of this process's console association.
    let state = if unsafe { GetConsoleWindow() }.is_null() { "none" } else { "attached" };
    println!("hook-console={state}");
    eprintln!("hook-pipe-stderr");
}

async fn exercise_console_policy(name: &str) -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    if std::env::var_os(PARENT).is_none() {
        let mut parent = tokio::process::Command::new(&executable);
        parent.args(["--exact", &format!("engine::command_runner::windows_console_tests::{name}"), "--nocapture"])
            .creation_flags(0x0000_0008) // DETACHED_PROCESS
            .env(PARENT, "1").kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(20), parent.output()).await??;
        assert!(output.status.success(), "detached hook regression failed: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("hook-console-check-complete"));
        return Ok(());
    }
    // SAFETY: Read-only query of the detached test parent's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let environment = Arc::new(std::env::vars_os().collect::<Vec<_>>());
    let shell = CommandShell {
        program: executable.to_string_lossy().into_owned(),
        args: vec!["--exact".into(), PROBE.into(), "--ignored".into(), "--nocapture".into()],
    };
    let (stdout, stderr) = match name {
        "contained_command_hook_has_no_console" => {
            let temporary = tempfile::tempdir()?;
            let (sender, _receiver) = async_channel::unbounded();
            let runtime = CommandHookRuntime::new(shell, environment, ThreadId::new(), sender);
            let handler = ConfiguredHandler {
                builtin: false, event_name: HookEventName::UserPromptSubmit, matcher: None,
                timeout_sec: 10, status_message: None, additional_context_limit: Default::default(),
                source_path: AbsolutePathBuf::try_from(temporary.path().join("hooks.json"))?.into(),
                source: HookSource::User, display_order: 0,
                kind: ConfiguredHandlerKind::Command { command: "--test-threads=1".into(), r#async: false, env: HashMap::new() },
            };
            let result = super::run_command(&runtime, &handler, "--test-threads=1", &HashMap::new(), "{}", temporary.path()).await;
            assert_eq!(result.exit_code, Some(0), "hook execution failed: {:?}", result.error);
            (result.stdout, result.stderr)
        }
        "uncontained_command_hook_has_no_console" => {
            let output = super::build_command(&shell, "--test-threads=1", &environment, &HashMap::new())
                .kill_on_drop(true).output().await?;
            assert!(output.status.success(), "hook execution failed: {output:?}");
            (String::from_utf8(output.stdout)?, String::from_utf8(output.stderr)?)
        }
        "legacy_notification_has_no_console" => {
            let mut argv = vec![shell.program];
            argv.extend(shell.args);
            let mut command = crate::registry::command_from_argv(&argv, environment.iter().cloned())
                .ok_or_else(|| anyhow::anyhow!("notification command was absent"))?;
            let output = command.kill_on_drop(true).output().await?;
            assert!(output.status.success(), "notification failed: {output:?}");
            (String::from_utf8(output.stdout)?, String::from_utf8(output.stderr)?)
        }
        _ => anyhow::bail!("unknown test scenario"),
    };
    assert!(stderr.contains("hook-pipe-stderr"));
    assert!(stdout.contains("hook-console=none"), "hook acquired a console: {stdout}");
    println!("hook-console-check-complete");
    Ok(())
}

#[tokio::test]
async fn contained_command_hook_has_no_console() -> anyhow::Result<()> {
    exercise_console_policy("contained_command_hook_has_no_console").await
}
#[tokio::test]
async fn uncontained_command_hook_has_no_console() -> anyhow::Result<()> {
    exercise_console_policy("uncontained_command_hook_has_no_console").await
}
#[tokio::test]
async fn legacy_notification_has_no_console() -> anyhow::Result<()> {
    exercise_console_policy("legacy_notification_has_no_console").await
}
