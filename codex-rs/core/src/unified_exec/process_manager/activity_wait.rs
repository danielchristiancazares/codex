use super::*;

/// Observing readiness leaves the shared output buffer and interaction lock untouched.
pub(crate) struct ProcessOutputReady;

impl UnifiedExecProcessManager {
    pub(crate) async fn wait_for_output(
        &self,
        process_id: i32,
    ) -> Result<ProcessOutputReady, UnifiedExecError> {
        let process = {
            let store = self.process_store.lock().await;
            Arc::clone(
                &store
                    .processes
                    .get(&process_id)
                    .ok_or(UnifiedExecError::UnknownProcessId { process_id })?
                    .process,
            )
        };
        Ok(process.output_handles().wait_for_activity().await)
    }
}

impl<const MAX_BYTES: usize> OutputHandles<MAX_BYTES> {
    async fn wait_for_activity(&self) -> ProcessOutputReady {
        loop {
            let notified = self.output_notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let buffer = self.output_buffer.lock().await;
                if buffer.total_bytes() > 0 || self.cancellation_token.is_cancelled() {
                    return ProcessOutputReady;
                }
            }
            tokio::select! {
                _ = &mut notified => {}
                _ = self.cancellation_token.cancelled() => return ProcessOutputReady,
            }
        }
    }
}

#[cfg(test)]
#[path = "activity_wait_tests.rs"]
mod tests;
