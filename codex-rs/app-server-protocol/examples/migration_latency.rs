//! Measures the production delta mapper with prebuilt event batches.
//! Run in release mode; fixture construction and destruction are outside timing.

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::item_event_to_server_notification;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::EventMsg;
use std::hint::black_box;
use std::time::Instant;

const THREAD: &str = "thread-01234567-89ab-cdef-0123-456789abcdef";
const TURN: &str = "turn-01234567-89ab-cdef-0123-456789abcdef";
const BATCH_SIZE: usize = 50_000;

fn main() {
    let fixture = EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
        thread_id: THREAD.to_string(),
        turn_id: TURN.to_string(),
        item_id: "item-01234567-89ab-cdef-0123-456789abcdef".to_string(),
        delta: "abcdefghijklmnop".to_string(),
    });
    let expected = serde_json::json!({
        "method": "item/agentMessage/delta",
        "params": {
            "threadId": THREAD,
            "turnId": TURN,
            "itemId": "item-01234567-89ab-cdef-0123-456789abcdef",
            "delta": "abcdefghijklmnop",
        }
    });
    assert_eq!(
        serde_json::to_value(item_event_to_server_notification(
            fixture.clone(),
            THREAD,
            TURN
        ))
        .expect("serialize notification"),
        expected,
    );
    let mut samples = Vec::new();
    for sample in 0..36 {
        let events = vec![fixture.clone(); BATCH_SIZE];
        let mut notifications: Vec<ServerNotification> = Vec::with_capacity(BATCH_SIZE);
        let start = Instant::now();
        for event in events {
            notifications.push(item_event_to_server_notification(
                black_box(event),
                black_box(THREAD),
                black_box(TURN),
            ));
        }
        let ns = start.elapsed().as_nanos() as f64 / BATCH_SIZE as f64;
        black_box(&notifications);
        if sample >= 5 {
            samples.push(ns);
        }
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "delta_mapping_ns_per_event={:.3}",
        samples[samples.len() / 2]
    );
}
