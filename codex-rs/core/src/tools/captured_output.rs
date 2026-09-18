//! Registers session-local captures before adapting tool output for the model.

mod read_output;
pub(crate) use read_output::ReadOutputHandler;

use super::context::ExecCommandToolOutput;
use crate::session::session::Session;
use codex_tools::CapturedOutput;
use codex_utils_output_truncation::CaptureId;

pub(crate) async fn capture<T>(
    session: &Session,
    original: T,
    text: String,
    omitted_bytes: usize,
) -> CapturedOutput<T> {
    let receipt = session.services.captured_output.lock().await.insert(
        CaptureId::new(uuid::Uuid::new_v4().into_bytes()),
        text,
        omitted_bytes,
    );
    CapturedOutput::new(original, receipt)
}

pub(crate) async fn capture_terminal(
    session: &Session,
    original: ExecCommandToolOutput,
) -> CapturedOutput<ExecCommandToolOutput> {
    let text = String::from_utf8_lossy(&original.raw_output).into_owned();
    let omitted_bytes = original
        .output_omitted_bytes
        .map_or(0, std::num::NonZeroUsize::get);
    capture(session, original, text, omitted_bytes).await
}

#[cfg(test)]
#[path = "captured_output_tests.rs"]
mod tests;
