use std::os::windows::process::CommandExt;
use std::process::Command;

const CHILD: &str = "CODEX_GIT_CONSOLE_TEST_PARENT";
const PROBE: &str = "command::windows_tests::console_probe";

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

#[test]
#[ignore = "runs as a child process of the console regression"]
fn console_probe() {
    // SAFETY: This only queries the current process's console association.
    let console = if unsafe { GetConsoleWindow() }.is_null() {
        "none"
    } else {
        "attached"
    };
    println!("git-console-probe={console}");
    eprintln!("git-console-stderr");
}

#[test]
fn background_git_does_not_allocate_console_from_detached_parent() -> anyhow::Result<()> {
    const TEST: &str =
        "command::windows_tests::background_git_does_not_allocate_console_from_detached_parent";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe()?)
            .creation_flags(0x0000_0008) // DETACHED_PROCESS
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .output()?;
        assert!(output.status.success(), "detached test failed: {output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("git-console-check-completed"));
        return Ok(());
    }
    // SAFETY: This only queries the current process's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let program = std::env::current_exe()?;
    let args = ["--exact", PROBE, "--ignored", "--nocapture"];
    let control = Command::new(&program).args(args).output()?;
    assert!(control.status.success(), "control failed: {control:?}");
    assert!(String::from_utf8_lossy(&control.stdout).contains("git-console-probe=attached"));
    let output = super::git_command(&program).args(args).output()?;
    assert!(output.status.success(), "Git helper failed: {output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("git-console-stderr"));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("git-console-probe=none"),
        "Git helper allocated a console: {output:?}"
    );
    println!("git-console-check-completed");
    Ok(())
}
