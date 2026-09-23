use std::time::Duration;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

#[tokio::test]
async fn credential_export_does_not_allocate_console() -> std::io::Result<()> {
    const PARENT: &str = "CODEX_HELPER_CONSOLE_PARENT";
    if std::env::var_os(PARENT).is_none() {
        let mut parent = tokio::process::Command::new(std::env::current_exe()?);
        parent.args(["--exact", "amazon_bedrock::credential_export::windows_console_tests::credential_export_does_not_allocate_console", "--nocapture"])
            .env(PARENT, "1").creation_flags(0x0000_0008).kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(30), parent.output()).await??;
        assert!(
            output.status.success(),
            "detached helper regression failed: {output:?}"
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("console-check-completed"));
        return Ok(());
    }
    // SAFETY: Read-only query of the detached parent's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let temporary = tempfile::tempdir()?;
    let script = temporary.path().join("console-probe.ps1");
    std::fs::write(
        &script,
        r#"$ErrorActionPreference = 'Stop'
Add-Type 'using System; using System.Runtime.InteropServices; public static class ConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }'
$state = if ([ConsoleProbe]::GetConsoleWindow() -eq [IntPtr]::Zero) { 'none' } else { 'attached' }
@{ AccessKeyId = $state; SecretAccessKey = 'test-only-placeholder' } | ConvertTo-Json -Compress
"#,
    )?;
    let system_root = std::env::var_os("SystemRoot")
        .ok_or_else(|| std::io::Error::other("SystemRoot missing"))?;
    let powershell = std::path::PathBuf::from(system_root)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let exporter =
        super::AwsCredentialExport::new(codex_model_provider_info::AwsCredentialExportConfig {
            command: powershell.to_string_lossy().into_owned(),
            args: [
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-File".to_owned(),
                script.to_string_lossy().into_owned(),
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
            timeout_ms: std::num::NonZeroU64::new(10_000)
                .ok_or_else(|| std::io::Error::other("timeout"))?,
        });
    pretty_assertions::assert_eq!(
        exporter
            .export_credentials()
            .await?
            .access_keys
            .access_key_id,
        "none"
    );
    println!("console-check-completed");
    Ok(())
}
