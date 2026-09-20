//! Native Windows coverage for console policy and suspended job assignment.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use pretty_assertions::assert_eq;
use tokio::io::AsyncWriteExt;
use winapi::um::winbase::DETACHED_PROCESS;
use winapi::um::wincon::GetConsoleWindow;

use super::Command;
use crate::JobObject;

const PARENT_ENV: &str = "CODEX_WINDOWS_CONSOLE_TEST_PARENT";
const PROBE_DIRECTORY: &str = "CODEX_WINDOWS_CHILD_CONSOLE_PROBE";
const PROBE_TEST: &str = "child_command::windows_tests::console_probe_child";

#[test]
#[ignore = "launched in a subprocess by console-policy tests"]
fn console_probe_child() -> Result<()> {
    let directory = PathBuf::from(std::env::var_os(PROBE_DIRECTORY).expect("probe directory"));
    // SAFETY: This only queries this process's console association.
    let attached = !unsafe { GetConsoleWindow() }.is_null();
    std::fs::write(
        directory.join("console"),
        if attached { "attached" } else { "none" },
    )?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    std::io::stdout().write_all(format!("probe-stdout {line}").as_bytes())?;
    std::io::stderr().write_all(b"probe-stderr\n")?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Launch {
    WithoutJob,
    NoConsoleBeforeJob,
    NoConsoleAfterJob,
    DefaultConsole,
}

async fn check_launch(launch: Launch) -> Result<()> {
    if std::env::var_os(PARENT_ENV).is_none() {
        let test = match launch {
            Launch::WithoutJob => "no_console_without_job_preserves_piped_stdio",
            Launch::NoConsoleBeforeJob => "no_console_survives_job_preparation",
            Launch::NoConsoleAfterJob => "no_console_after_job_preparation_preserves_suspension",
            Launch::DefaultConsole => "default_command_keeps_console_behavior",
        };
        let mut parent = tokio::process::Command::new(std::env::current_exe()?);
        parent
            .args([
                "--exact",
                &format!("child_command::windows_tests::{test}"),
                "--nocapture",
            ])
            .env(PARENT_ENV, "1")
            .creation_flags(DETACHED_PROCESS)
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(30), parent.output()).await??;
        assert!(
            output.status.success(),
            "detached parent failed for {launch:?}: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("console-policy-check-completed"),
            "child test did not run: {output:?}"
        );
        return Ok(());
    }
    // SAFETY: This only queries this process's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let temp = tempfile::tempdir()?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--exact", PROBE_TEST, "--ignored", "--nocapture"])
        .envs(std::env::vars_os())
        .env(PROBE_DIRECTORY, temp.path());
    let job = match launch {
        Launch::WithoutJob => {
            command.no_console();
            None
        }
        Launch::NoConsoleBeforeJob => {
            let job = JobObject::create_without_breakaway()?;
            command.no_console();
            command.prepare_suspended_spawn(&job);
            Some(job)
        }
        Launch::NoConsoleAfterJob => {
            let job = JobObject::create_without_breakaway()?;
            command.prepare_suspended_spawn(&job);
            command.no_console();
            Some(job)
        }
        Launch::DefaultConsole => None,
    };
    let mut child = command.spawn()?;
    if let Some(job) = &job {
        assert!(
            !temp.path().join("console").exists(),
            "child ran before job assignment"
        );
        assert!(
            job.assign_and_resume_process(
                child
                    .id()
                    .ok_or_else(|| anyhow::anyhow!("suspended child id"))?
            )?
        );
    }
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("piped stdin"))?;
    stdin.write_all(b"pipe round trip\n").await?;
    drop(stdin);
    let output = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output()).await??;
    assert!(
        output.status.success(),
        "probe failed for {launch:?}: {output:?}"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("probe-stdout pipe round trip"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("probe-stderr"));
    let expected = match launch {
        Launch::DefaultConsole => "attached",
        Launch::WithoutJob | Launch::NoConsoleBeforeJob | Launch::NoConsoleAfterJob => "none",
    };
    assert_eq!(
        std::fs::read_to_string(temp.path().join("console"))?,
        expected,
        "{launch:?}"
    );
    println!("console-policy-check-completed");
    Ok(())
}

#[tokio::test]
async fn no_console_without_job_preserves_piped_stdio() -> Result<()> {
    check_launch(Launch::WithoutJob).await
}

#[tokio::test]
async fn no_console_survives_job_preparation() -> Result<()> {
    check_launch(Launch::NoConsoleBeforeJob).await
}

#[tokio::test]
async fn no_console_after_job_preparation_preserves_suspension() -> Result<()> {
    check_launch(Launch::NoConsoleAfterJob).await
}

#[tokio::test]
async fn default_command_keeps_console_behavior() -> Result<()> {
    check_launch(Launch::DefaultConsole).await
}
