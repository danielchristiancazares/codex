use super::SpawnChildRequest;
use super::StdioPolicy;
use super::spawn_child_async;
use std::time::Duration;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

#[test]
#[ignore = "native console probe subprocess"]
fn console_probe() -> std::io::Result<()> {
    let path = std::env::var_os("CODEX_SPAWN_CONSOLE_REPORT")
        .ok_or_else(|| std::io::Error::other("report path"))?;
    // SAFETY: This only queries this process's console association.
    std::fs::write(
        path,
        if unsafe { GetConsoleWindow() }.is_null() {
            "none"
        } else {
            "attached"
        },
    )?;
    println!("spawn-pipe-output");
    Ok(())
}

async fn exercise(name: &str, policy: StdioPolicy) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    if std::env::var_os("CODEX_SPAWN_CONSOLE_PARENT").is_none() {
        let mut command = tokio::process::Command::new(&exe);
        command
            .args([
                "--exact",
                &format!("spawn::windows_console_tests::{name}"),
                "--nocapture",
            ])
            .env("CODEX_SPAWN_CONSOLE_PARENT", "1")
            .creation_flags(0x0000_0008)
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(20), command.output()).await??;
        assert!(
            output.status.success(),
            "detached spawn test failed: {output:?}"
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("spawn-console-check-complete"));
        return Ok(());
    }
    // SAFETY: This only queries the detached parent's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let temp = tempfile::tempdir()?;
    let report = temp.path().join("console");
    let mut env = std::env::vars().collect::<std::collections::HashMap<_, _>>();
    env.insert(
        "CODEX_SPAWN_CONSOLE_REPORT".to_owned(),
        report.to_string_lossy().into_owned(),
    );
    let child = spawn_child_async(SpawnChildRequest {
        program: exe,
        args: [
            "--exact",
            "spawn::windows_console_tests::console_probe",
            "--ignored",
            "--nocapture",
        ]
        .map(str::to_owned)
        .to_vec(),
        arg0: None,
        cwd: temp.path().to_path_buf().try_into()?,
        network_sandbox_policy: codex_protocol::permissions::NetworkSandboxPolicy::Enabled,
        network: None,
        stdio_policy: policy,
        env,
    })
    .await?;
    let output = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output()).await??;
    assert!(output.status.success(), "spawn failed: {output:?}");
    let expected = match policy {
        StdioPolicy::RedirectForShellTool => "none",
        StdioPolicy::Inherit => "attached",
    };
    pretty_assertions::assert_eq!(std::fs::read_to_string(report)?, expected);
    if matches!(policy, StdioPolicy::RedirectForShellTool) {
        assert!(String::from_utf8_lossy(&output.stdout).contains("spawn-pipe-output"));
    }
    println!("spawn-console-check-complete");
    Ok(())
}

#[tokio::test]
async fn redirected_shell_does_not_allocate_console() -> anyhow::Result<()> {
    exercise(
        "redirected_shell_does_not_allocate_console",
        StdioPolicy::RedirectForShellTool,
    )
    .await
}
#[tokio::test]
async fn inherited_stdio_preserves_console_behavior() -> anyhow::Result<()> {
    exercise(
        "inherited_stdio_preserves_console_behavior",
        StdioPolicy::Inherit,
    )
    .await
}
