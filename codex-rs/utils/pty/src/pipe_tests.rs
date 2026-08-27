use super::*;
use pretty_assertions::assert_eq;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::ReadBuf;

#[cfg(windows)]
#[test]
fn process_fallback_interrupt_terminates_root() -> anyhow::Result<()> {
    let mut child = std::process::Command::new("ping.exe")
        .args(["-n", "60", "127.0.0.1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut terminator = PipeChildTerminator {
        windows: WindowsChildTerminator::Process(child.id()),
    };

    terminator.signal(ProcessSignal::Interrupt)?;

    assert!(!child.wait()?.success());
    Ok(())
}

struct FailingReader {
    prefix: Option<&'static [u8]>,
}

impl AsyncRead for FailingReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Some(prefix) = this.prefix.take() {
            buf.put_slice(prefix);
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(io::Error::other("injected read failure")))
        }
    }
}

#[tokio::test]
async fn read_error_marks_output_lost_after_the_valid_prefix() {
    let (output_tx, mut output_rx) = mpsc::channel(2);
    let output_lost = Arc::new(AtomicBool::new(false));

    read_output_stream(
        FailingReader {
            prefix: Some(b"prefix"),
        },
        output_tx,
        Arc::clone(&output_lost),
    )
    .await;

    let mut output = Vec::new();
    while let Some(chunk) = output_rx.recv().await {
        output.extend_from_slice(&chunk);
    }
    assert_eq!(output, b"prefix");
    assert!(output_lost.load(Ordering::Acquire));
}
