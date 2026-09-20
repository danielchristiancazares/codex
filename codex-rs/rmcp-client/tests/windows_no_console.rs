#![cfg(windows)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;

const CHILD_ENV: &str = "CODEX_MCP_CONSOLE_TEST_CHILD";
const DETACHED_PROCESS: u32 = 0x0000_0008;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
}

#[derive(Clone, Copy)]
enum ServerCommand {
    Executable,
    BatchWrapper,
}

async fn check_consoleless_launch(test_name: &str, server_command: ServerCommand) -> Result<()> {
    if std::env::var_os(CHILD_ENV).is_none() {
        // Reexecute this test as a consoleless parent, matching a desktop app.
        let mut command = tokio::process::Command::new(std::env::current_exe()?);
        command
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD_ENV, "1")
            .creation_flags(DETACHED_PROCESS)
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(30), command.output()).await??;
        assert!(
            output.status.success(),
            "consoleless launcher failed\nstdout {}\nstderr {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("console-probe-completed"),
            "child test did not execute: {output:?}"
        );
        return Ok(());
    }

    // SAFETY: This only queries the current process's console association.
    assert!(unsafe { GetConsoleWindow() }.is_null());
    let temp = tempfile::tempdir()?;
    let report = temp.path().join("console-state");
    let server = codex_utils_cargo_bin::cargo_bin("test_stdio_server")?;
    let mut env = HashMap::from([
        (
            OsString::from("MCP_TEST_CONSOLE_STATE_FILE"),
            report.clone().into_os_string(),
        ),
        (
            OsString::from("MCP_TEST_VALUE"),
            OsString::from("consoleless-env"),
        ),
    ]);
    let program = match server_command {
        ServerCommand::Executable => server,
        ServerCommand::BatchWrapper => {
            let wrapper = temp.path().join("server with spaces.cmd");
            std::fs::write(
                &wrapper,
                "@echo off\r\n\"%MCP_CONSOLE_TEST_SERVER%\" %*\r\n",
            )?;
            env.insert(
                OsString::from("MCP_CONSOLE_TEST_SERVER"),
                server.into_os_string(),
            );
            wrapper
        }
    };
    let client = RmcpClient::new_stdio_client(
        program.into_os_string(),
        Vec::new(),
        Some(env),
        &[],
        Some(temp.path().to_string_lossy().into_owned()),
        Arc::new(LocalStdioServerLauncher::new(temp.path().to_path_buf())),
    )
    .await?;
    client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("console-test", "1"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            Some(Duration::from_secs(10)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Decline,
                        content: None,
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;
    let result = client
        .call_tool(
            "echo".to_string(),
            Some(json!({"message": "pipe round trip"})),
            /*meta*/ None,
            Some(Duration::from_secs(10)),
        )
        .await?;
    assert_eq!(
        result.structured_content,
        Some(json!({"echo": "ECHOING: pipe round trip", "env": "consoleless-env"}))
    );
    let console = std::fs::read_to_string(report)?;
    client.shutdown().await;
    println!("console-probe-completed, observed {console}");
    assert_eq!(
        console, "none",
        "MCP server acquired a console despite its piped transport"
    );
    Ok(())
}

#[tokio::test]
async fn executable_mcp_from_consoleless_parent_has_no_console() -> Result<()> {
    check_consoleless_launch(
        "executable_mcp_from_consoleless_parent_has_no_console",
        ServerCommand::Executable,
    )
    .await
}

#[tokio::test]
async fn batch_mcp_from_consoleless_parent_has_no_console() -> Result<()> {
    check_consoleless_launch(
        "batch_mcp_from_consoleless_parent_has_no_console",
        ServerCommand::BatchWrapper,
    )
    .await
}
