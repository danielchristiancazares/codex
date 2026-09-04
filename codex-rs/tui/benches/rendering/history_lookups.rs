//! Production-path microbenchmarks for upstream history lookup backports.

use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadItem;
use codex_protocol::ThreadId;
use codex_protocol::items::PlanItem;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use divan::Bencher;

#[path = "../../src/bottom_pane/chat_composer/history_match_ranges.rs"]
// Cargo enables cfg(test) for this harness-free benchmark; libtest functions
// remain inactive while the production mapper is measured.
#[allow(unused_imports)]
mod history_match_ranges;

#[divan::bench(args = [16, 256, 4096], sample_count = 50, sample_size = 20)]
fn unicode_highlight_ranges(bencher: Bencher, repeats: usize) {
    let text = "İ a ".repeat(repeats);
    bencher.bench_local(|| {
        history_match_ranges::case_insensitive_match_ranges(
            divan::black_box(&text),
            divan::black_box("i"),
        )
    });
}

#[divan::bench(args = [16, 256, 4096], sample_count = 50, sample_size = 20)]
fn unicode_highlight_no_match(bencher: Bencher, repeats: usize) {
    let text = "İ a ".repeat(repeats);
    bencher.bench_local(|| {
        history_match_ranges::case_insensitive_match_ranges(
            divan::black_box(&text),
            divan::black_box("z"),
        )
    });
}

#[divan::bench(args = [8, 32, 128, 1024], sample_count = 50, sample_size = 20)]
fn thread_history_append(bencher: Bencher, count: usize) {
    let events = item_events(count);
    bencher
        .with_inputs(new_history_builder)
        .bench_local_values(|mut builder| {
            for event in divan::black_box(&events) {
                builder.handle_event(event);
            }
            builder.finish()
        });
}

#[divan::bench(args = [8, 32, 128, 1024], sample_count = 50, sample_size = 20)]
fn thread_history_late_updates(bencher: Bencher, count: usize) {
    let events = item_events(count);
    bencher
        .with_inputs(|| {
            let mut builder = new_history_builder();
            for event in &events {
                builder.handle_event(event);
            }
            builder.handle_event(&EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "history-benchmark".to_string(),
                started_at: None,
                last_agent_message: None,
                error: None,
                completed_at: None,
                duration_ms: None,
                time_to_first_token_ms: None,
            }));
            builder
        })
        .bench_local_values(|mut builder| {
            for event in divan::black_box(&events).iter().rev() {
                builder.handle_event(event);
            }
            builder.finish()
        });
}

fn item_events(count: usize) -> Vec<EventMsg> {
    let thread_id = ThreadId::new();
    let events: Vec<_> = (0..count)
        .map(|index| {
            EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id,
                turn_id: "history-benchmark".to_string(),
                item: TurnItem::Plan(PlanItem {
                    id: format!("plan-{index}"),
                    text: "Inspect, implement, validate.".to_string(),
                }),
                started_at_ms: Some(0),
                completed_at_ms: 1,
            })
        })
        .collect();
    let mut builder = new_history_builder();
    for event in &events {
        builder.handle_event(event);
    }
    let expected: Vec<_> = (0..count)
        .map(|index| ThreadItem::Plan {
            id: format!("plan-{index}"),
            text: "Inspect, implement, validate.".to_string(),
        })
        .collect();
    assert_eq!(builder.finish()[0].items, expected);
    events
}

fn new_history_builder() -> ThreadHistoryBuilder {
    let mut builder = ThreadHistoryBuilder::new();
    builder.handle_event(&EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: "history-benchmark".to_string(),
        trace_id: None,
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }));
    builder
}
