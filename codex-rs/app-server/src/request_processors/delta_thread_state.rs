use codex_protocol::protocol::EventMsg;

#[cfg(test)]
#[path = "delta_thread_state_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DeltaThreadStateTracking;

impl DeltaThreadStateTracking {
    pub(super) fn requires_thread_state(&self, event: &EventMsg) -> bool {
        match event {
            EventMsg::AgentMessageContentDelta(_)
            | EventMsg::PlanDelta(_)
            | EventMsg::ReasoningContentDelta(_)
            | EventMsg::ReasoningRawContentDelta(_)
            | EventMsg::AgentReasoningSectionBreak(_) => false,
            // New event kinds retain state tracking until their invariants are established here.
            _ => true,
        }
    }
}
