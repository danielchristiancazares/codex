#[cfg(unix)]
use std::path::Path;
use std::time::Duration;

use codex_utils_pty::ProcessDriver;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::spawn_from_driver;
use pretty_assertions::assert_eq;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;

use super::MAX_QUERY_BYTES;
use super::TerminalQueryResponder;
use super::respond_to_terminal_queries;
#[cfg(unix)]
use crate::SandboxType;
#[cfg(unix)]
use crate::SpawnRequest;
#[cfg(unix)]
use crate::spawn_process;

#[tokio::test]
async fn driver_backed_terminal_queries_are_answered() -> anyhow::Result<()> {
    let (writer_tx, mut writer_rx) = mpsc::channel(/*buffer*/ 8);
    let (stdout_tx, stdout_rx) = broadcast::channel(/*capacity*/ 8);
    let (exit_tx, exit_rx) = oneshot::channel();
    let spawned = respond_to_terminal_queries(spawn_from_driver(ProcessDriver {
        writer_tx,
        stdout_rx,
        stderr_rx: None,
        exit_rx,
        terminator: None,
        writer_handle: None,
        resizer: None,
        #[cfg(windows)]
        tty: true,
    }));
    let SpawnedProcess {
        session: _session,
        mut stdout_rx,
        ..
    } = spawned;

    stdout_tx.send(b"before\x1b[".to_vec())?;
    stdout_tx.send(b"5n\x1b[18t\x1b[6n\x1b[?1049$p\x1b[31mafter\x1b[".to_vec())?;
    let responses = timeout(Duration::from_secs(/*secs*/ 2), writer_rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("terminal response channel closed"))?;
    assert_eq!(responses, b"\x1b[0n\x1b[8;24;80t\x1b[1;1R\x1b[?1049;0$y");

    drop(stdout_tx);
    exit_tx.send(/*t*/ 0).expect("send exit code");
    let output = timeout(Duration::from_secs(/*secs*/ 2), async move {
        let mut output = Vec::new();
        while let Some(chunk) = stdout_rx.recv().await {
            output.extend(chunk);
        }
        output
    })
    .await?;
    assert_eq!(output, b"before\x1b[31mafter\x1b[".to_vec());

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn direct_terminal_queries_are_answered() -> anyhow::Result<()> {
    let script =
        "stty -echo -icanon; printf 'alpha\\033[6n'; dd bs=1 count=6 2>/dev/null; printf '\\nok'";
    let command = vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()];
    let env = std::env::vars().collect();
    let spawned = spawn_process(SpawnRequest {
        command: &command,
        cwd: Path::new("."),
        env: &env,
        arg0: &None,
        sandbox: SandboxType::None,
        windows_sandbox: None,
        tty: true,
        stdin_open: true,
        inherited_fds: &[],
    })
    .await?;
    let SpawnedProcess {
        session: _session,
        mut stdout_rx,
        exit_rx,
        ..
    } = spawned;
    let (code, output) = timeout(Duration::from_secs(/*secs*/ 5), async move {
        let code = exit_rx.await?;
        let mut output = Vec::new();
        while let Some(chunk) = stdout_rx.recv().await {
            output.extend(chunk);
        }
        anyhow::Ok((code, output))
    })
    .await??;

    assert_eq!((code, output), (0, b"alpha\x1b[1;1R\r\nok".to_vec()));

    Ok(())
}

#[test]
fn unhandled_sequences_preserve_bytes_across_chunk_boundaries() {
    let raw = b"left\x1b[?12345678901$p\x1b[2J\x1b]0;title\x07right";
    for split in 0..=raw.len() {
        let mut responder = TerminalQueryResponder::default();
        let (mut output, mut responses) = responder.process(raw[..split].to_vec());
        assert!(responder.pending.len() < MAX_QUERY_BYTES);
        let (tail, tail_responses) = responder.process(raw[split..].to_vec());
        output.extend(tail);
        responses.extend(tail_responses);
        assert_eq!(
            (output, responses, responder.pending),
            (raw.to_vec(), Vec::new(), Vec::new()),
        );
    }
}
