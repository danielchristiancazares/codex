use super::*;

#[test]
fn context_window_hint_is_utf8_bounded_with_marker() {
    let context = TokenBudgetContext::new(
        AgentPath::root(),
        Uuid::nil(),
        /*previous_window_id*/ None,
        Uuid::from_u128(u128::MAX),
        Some(format!("head-{}-tail", "🦀".repeat(2_000))),
    );
    let hint = context.thread_hint.expect("bounded hint");

    assert!(hint.len() <= CONTEXT_WINDOW_HINT_MAX_BYTES);
    assert!(hint.starts_with("head-"));
    assert!(hint.ends_with(CONTEXT_WINDOW_HINT_TRUNCATION_MARKER));
    assert!(!hint.contains("-tail"));
}
