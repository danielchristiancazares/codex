use super::ThreadHistoryBuilder;
use super::ThreadItem;
use super::item_index::TURN_ITEM_INDEX_THRESHOLD;
use super::item_index::TurnItemIndex;
use codex_extension_items::sleep::SleepItem as CoreSleepItem;
use codex_protocol::protocol::ThreadRolledBackEvent;
use pretty_assertions::assert_eq;

fn indexed_sleep_item(id: &str, duration_ms: u64) -> ThreadItem {
    ThreadItem::Sleep(CoreSleepItem {
        id: id.to_string(),
        duration_ms,
    })
}

#[test]
fn pushed_duplicates_resolve_to_first_occurrence_before_and_after_indexing() {
    for count in [2, TURN_ITEM_INDEX_THRESHOLD, TURN_ITEM_INDEX_THRESHOLD * 2] {
        let mut index = TurnItemIndex::default();
        let mut items = Vec::new();
        let original = indexed_sleep_item("duplicate", /*duration_ms*/ 1);
        for _ in 0..count {
            index.push(&mut items, original.clone());
        }
        let updated = indexed_sleep_item("duplicate", /*duration_ms*/ 2);
        index.upsert(&mut items, updated.clone());
        // Appends after index activation must also keep the first position.
        index.push(&mut items, original.clone());
        index.upsert(&mut items, updated.clone());
        let appended = indexed_sleep_item("appended", /*duration_ms*/ 3);
        index.push(
            &mut items,
            indexed_sleep_item("appended", /*duration_ms*/ 1),
        );
        index.upsert(&mut items, appended.clone());

        let mut expected = vec![original; count + 1];
        expected[0] = updated;
        expected.push(appended);
        assert_eq!(items, expected);
    }
}

#[test]
fn indexed_items_stay_with_their_turn_after_rollback() {
    let mut builder = ThreadHistoryBuilder::new();
    let mut expected: Vec<_> = (0..64)
        .map(|i| indexed_sleep_item(&i.to_string(), /*duration_ms*/ 1))
        .collect();
    for turn_id in ["retained", "discarded"] {
        builder.ensure_turn().id = turn_id.into();
        for item in &expected {
            builder.upsert_item_in_current_turn(item.clone());
        }
        builder.finish_current_turn();
        // Reuse IDs at different positions in the other turn.
        expected.reverse();
    }
    builder.handle_thread_rollback(&ThreadRolledBackEvent { num_turns: 1 });
    expected[0] = indexed_sleep_item("0", /*duration_ms*/ 2);
    builder.upsert_item_in_turn_id("retained", expected[0].clone());
    assert_eq!(builder.finish()[0].items, expected);
}
