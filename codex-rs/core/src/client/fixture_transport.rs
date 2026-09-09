//! Explicit authority for legacy SSE integration fixtures. Production uses WebSockets.

use codex_protocol::error::CodexErr;
use std::sync::OnceLock;

pub(super) struct ResponsesSseFixture;

static SSE_FIXTURE: OnceLock<ResponsesSseFixture> = OnceLock::new();

impl ResponsesSseFixture {
    pub(super) fn acquire() -> Result<&'static Self, CodexErr> {
        SSE_FIXTURE.get().ok_or_else(|| {
            CodexErr::UnsupportedOperation(
                "This build requires Responses WebSocket support; HTTP/SSE inference is reserved for explicit integration fixtures.".to_string(),
            )
        })
    }
}

pub(crate) fn enable_responses_sse_for_tests() {
    let _ = SSE_FIXTURE.set(ResponsesSseFixture);
}
