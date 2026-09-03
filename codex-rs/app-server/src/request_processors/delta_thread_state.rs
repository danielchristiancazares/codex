use crate::thread_state::ThreadState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadHistoryMode;

#[cfg(test)]
#[path = "delta_thread_state_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RealtimeDeltaTracking {
    Disabled,
    AwaitingStart,
    Started,
}

impl RealtimeDeltaTracking {
    pub(super) fn for_listener(
        history_mode: ThreadHistoryMode,
        thread_state: &ThreadState,
    ) -> Self {
        match history_mode {
            ThreadHistoryMode::Legacy => Self::Disabled,
            ThreadHistoryMode::Paginated if thread_state.realtime_history_ever_started => {
                Self::Started
            }
            ThreadHistoryMode::Paginated => Self::AwaitingStart,
        }
    }

    pub(super) fn requires_thread_state(&mut self, event: &EventMsg) -> bool {
        match event {
            EventMsg::AgentMessageContentDelta(_) => matches!(self, Self::Started),
            EventMsg::PlanDelta(_)
            | EventMsg::ReasoningContentDelta(_)
            | EventMsg::ReasoningRawContentDelta(_)
            | EventMsg::AgentReasoningSectionBreak(_) => false,
            EventMsg::RealtimeConversationStarted(_) => {
                if matches!(self, Self::AwaitingStart) {
                    *self = Self::Started;
                }
                true
            }
            // New event kinds retain state tracking until their invariants are established here.
            _ => true,
        }
    }
}
