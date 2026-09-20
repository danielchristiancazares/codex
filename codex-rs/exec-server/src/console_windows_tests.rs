use std::collections::HashMap;
use std::time::Duration;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

const PARENT: &str = "CODEX_EXEC_STARTUP_CONSOLE_PARENT";
const PROBE: &str = "client_transport::windows_console_tests::console_probe";

#[test]
#[ignore = "subprocess used by console regressions"]
fn console_probe() {
    // SAFETY: Read-only query of this process's console association.
    let state = if unsafe { GetConsoleWindow() }.is_null() { "none" } else { "attached" };
    println!("executor-console={state}");
    eprintln!("executor-stderr");
}

async fn exercise(name: &str) -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    if std::env::var_os(PARENT).is_none() {
        let mut parent = tokio::process::Command::new(&executable);
        parent.args(["--exact", &format!("client_transport::windows_console_tests::{name}"), "--nocapture"])
            .env(PARENT, "1").creation_flags(0x0000_0008).kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(20), parent.output()).await??;
        assert!(output.status.success(), "executor console test failed: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("executor-console-check-complete"));
        return Ok(());
    }
    // SAFETY: Read-only query of the detached parent's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let args = ["--exact", PROBE, "--ignored", "--nocapture"].map(str::to_owned).to_vec();
    let output = if name == "stdio_exec_server_starts_without_console" {
        let specification = super::StdioExecServerCommand {
            program: executable.to_string_lossy().into_owned(), args, env: HashMap::new(), cwd: None,
        };
        super::stdio_command_process(&specification).kill_on_drop(true).output().await?
    } else {
        let cwd = codex_utils_path_uri::PathUri::from_host_native_path(std::env::current_dir()?)?;
        let request = codex_sandboxing::SandboxExecRequest {
            command: std::iter::once(executable.to_string_lossy().into_owned()).chain(args).collect(),
            cwd: cwd.clone(), sandbox_policy_cwd: cwd, env: std::env::vars().collect(),
            network: None, network_environment_id: None, sandbox: codex_sandboxing::SandboxType::None,
            windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel::Disabled,
            permission_profile: codex_protocol::models::PermissionProfile::Disabled, arg0: None,
        };
        crate::fs_sandbox::spawn_command(request, codex_utils_pty::ChildStdin::Piped)
            .map_err(|error| anyhow::anyhow!("{}", error.message))?.wait_with_output().await?
    };
    assert!(output.status.success(), "executor failed: {output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("executor-stderr"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("executor-console=none"), "executor allocated a console: {output:?}");
    println!("executor-console-check-complete");
    Ok(())
}

#[tokio::test]
async fn stdio_exec_server_starts_without_console() -> anyhow::Result<()> {
    exercise("stdio_exec_server_starts_without_console").await
}
#[tokio::test]
async fn filesystem_helper_starts_without_console() -> anyhow::Result<()> {
    exercise("filesystem_helper_starts_without_console").await
}
