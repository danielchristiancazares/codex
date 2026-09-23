use std::time::Duration;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

#[tokio::test]
async fn header_helper_keeps_consoleless_parent_and_json_output() -> anyhow::Result<()> {
    const CHILD: &str = "CODEX_HTTP_HELPER_CONSOLE_PARENT";
    const TEST: &str = "http_headers::windows_console_tests::header_helper_keeps_consoleless_parent_and_json_output";
    if std::env::var_os(CHILD).is_none() {
        let mut command = tokio::process::Command::new(std::env::current_exe()?);
        command
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .creation_flags(0x0000_0008)
            .kill_on_drop(true); // DETACHED_PROCESS
        let output = tokio::time::timeout(Duration::from_secs(30), command.output()).await??;
        assert!(
            output.status.success(),
            "HTTP header helper regression failed: {output:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("http-helper-console-check-complete")
        );
        return Ok(());
    }
    // SAFETY: This only reads the current process's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let temporary = tempfile::tempdir()?;
    let script = temporary.path().join("probe.ps1");
    std::fs::write(
        &script,
        r#"$ErrorActionPreference = 'Stop'
Add-Type 'using System; using System.Runtime.InteropServices; public static class ConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }'
$state = if ([ConsoleProbe]::GetConsoleWindow() -eq [IntPtr]::Zero) { 'none' } else { 'attached' }
@{ 'x-console-state' = $state } | ConvertTo-Json -Compress
"#,
    )?;
    let system_root =
        std::env::var_os("SystemRoot").ok_or_else(|| anyhow::anyhow!("SystemRoot is absent"))?;
    let powershell = std::path::PathBuf::from(system_root)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let command = format!(
        r#""{}" -NoLogo -NoProfile -NonInteractive -File "{}""#,
        powershell.display(),
        script.display()
    );
    let headers = super::run_helper(&command, temporary.path()).await?;
    pretty_assertions::assert_eq!(headers["x-console-state"], "none");
    println!("http-helper-console-check-complete");
    Ok(())
}
