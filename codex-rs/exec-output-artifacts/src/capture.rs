use crate::ArtifactCapture;
use std::sync::Mutex;

#[derive(Debug)]
pub struct ArtifactCaptureBuffer {
    byte_cap: usize,
    state: Mutex<CaptureState>,
}

#[derive(Debug)]
struct CaptureState {
    bytes: Vec<u8>,
    observed_byte_count: u64,
    capture: ArtifactCapture,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCaptureSnapshot {
    pub bytes: Vec<u8>,
    pub observed_byte_count: u64,
    pub capture: ArtifactCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactCaptureStatus {
    pub retained_byte_count: u64,
    pub observed_byte_count: u64,
    pub capture: ArtifactCapture,
}

impl ArtifactCaptureBuffer {
    pub fn new(byte_cap: usize) -> Self {
        Self {
            byte_cap,
            state: Mutex::new(CaptureState {
                bytes: Vec::new(),
                observed_byte_count: 0,
                capture: ArtifactCapture::Complete,
            }),
        }
    }

    pub fn push(&self, bytes: &[u8]) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.observed_byte_count = state
            .observed_byte_count
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let remaining = self.byte_cap.saturating_sub(state.bytes.len());
        let retained = remaining.min(bytes.len());
        state.bytes.extend_from_slice(&bytes[..retained]);
        if retained != bytes.len() {
            state.capture = ArtifactCapture::Truncated;
        }
    }

    pub fn mark_truncated(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .capture = ArtifactCapture::Truncated;
    }

    pub fn status(&self) -> ArtifactCaptureStatus {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ArtifactCaptureStatus {
            retained_byte_count: u64::try_from(state.bytes.len()).unwrap_or(u64::MAX),
            observed_byte_count: state.observed_byte_count,
            capture: state.capture,
        }
    }

    pub fn take(&self) -> ArtifactCaptureSnapshot {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ArtifactCaptureSnapshot {
            bytes: std::mem::take(&mut state.bytes),
            observed_byte_count: state.observed_byte_count,
            capture: state.capture,
        }
    }
}
