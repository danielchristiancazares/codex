use super::*;
use crate::thread_state::ThreadState;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentReasoningSectionBreakEvent;
use codex_protocol::protocol::PlanDeltaEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::ReasoningContentDeltaEvent;
use codex_protocol::protocol::ReasoningRawContentDeltaEvent;
use codex_protocol::protocol::TurnStartedEvent;
use pretty_assertions::assert_eq;

fn agent_delta() -> EventMsg {
    EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        delta: "hello".to_string(),
    })
}

fn realtime_started() -> EventMsg {
    EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
        realtime_session_id: Some("realtime-1".to_string()),
        version: RealtimeConversationVersion::V2,
    })
}

fn permanently_stateless_deltas() -> Vec<EventMsg> {
    vec![
        EventMsg::PlanDelta(PlanDeltaEvent {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            delta: "plan".to_string(),
        }),
        EventMsg::ReasoningContentDelta(ReasoningContentDeltaEvent {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            delta: "summary".to_string(),
            summary_index: 0,
        }),
        EventMsg::ReasoningRawContentDelta(ReasoningRawContentDeltaEvent {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            delta: "reasoning".to_string(),
            content_index: 0,
        }),
        EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {
            item_id: "item-1".to_string(),
            summary_index: 1,
        }),
    ]
}

#[test]
fn delta_tracking_bypasses_only_events_unused_by_thread_state() {
    let mut thread_state = ThreadState::default();
    thread_state.track_current_turn_event(
        "turn-1",
        &EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        }),
    );
    let active_turn = thread_state.active_turn_snapshot();
    for event in std::iter::once(agent_delta()).chain(permanently_stateless_deltas()) {
        thread_state.track_current_turn_event("turn-1", &event);
    }
    assert_eq!(thread_state.active_turn_snapshot(), active_turn);

    let tracking = DeltaThreadStateTracking;
    assert!(!tracking.requires_thread_state(&agent_delta()));
    assert!(tracking.requires_thread_state(&realtime_started()));
    assert!(tracking.requires_thread_state(&EventMsg::ShutdownComplete));

    let requirements = permanently_stateless_deltas()
        .iter()
        .map(|event| tracking.requires_thread_state(event))
        .collect::<Vec<_>>();
    assert_eq!(requirements, vec![false; 4]);
}

#[test]
fn realtime_start_does_not_disable_agent_delta_bypass() {
    let tracking = DeltaThreadStateTracking;
    assert!(tracking.requires_thread_state(&realtime_started()));
    assert!(!tracking.requires_thread_state(&agent_delta()));
}
