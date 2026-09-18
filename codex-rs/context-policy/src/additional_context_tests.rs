use std::collections::BTreeMap;

use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use pretty_assertions::assert_eq;

use super::AdditionalContextStore;

fn browser_context(value: &str) -> BTreeMap<String, AdditionalContextEntry> {
    BTreeMap::from([(
        "browser_info".to_string(),
        AdditionalContextEntry {
            value: value.to_string(),
            kind: AdditionalContextKind::Untrusted,
        },
    )])
}

#[test]
fn committed_snapshot_suppresses_an_unchanged_publication() {
    let mut store = AdditionalContextStore::default();
    let publication = browser_context("same tab");
    let (items, snapshot) = store.prepare(publication.clone());
    assert_eq!(items.len(), 1);
    store.commit(snapshot);

    let (items, _) = store.prepare(publication);
    assert_eq!(items, Vec::new());
}

#[test]
fn restored_snapshot_suppresses_an_unchanged_publication() {
    let mut original = AdditionalContextStore::default();
    let publication = browser_context("same tab");
    let (_, snapshot) = original.prepare(publication.clone());
    original.commit(snapshot.clone());

    let mut restored = AdditionalContextStore::default();
    restored.restore(snapshot);
    let (items, _) = restored.prepare(publication);
    assert_eq!(items, Vec::new());
}

#[test]
fn explicit_clear_allows_the_same_value_to_be_published_again() {
    let mut store = AdditionalContextStore::default();
    let publication = browser_context("same tab");
    let (_, snapshot) = store.prepare(publication.clone());
    store.commit(snapshot);
    let (_, cleared) = store.prepare(BTreeMap::new());
    store.commit(cleared);

    let (items, _) = store.prepare(publication);
    assert_eq!(items.len(), 1);
}

#[test]
fn render_equivalent_tail_changes_are_suppressed_after_truncation() {
    let mut store = AdditionalContextStore::default();
    let first = format!("head{}a{}tail", "x".repeat(20_000), "y".repeat(20_000));
    let second = format!("head{}b{}tail", "x".repeat(20_000), "y".repeat(20_000));
    let (_, snapshot) = store.prepare(browser_context(&first));
    store.commit(snapshot);

    let (items, _) = store.prepare(browser_context(&second));
    assert_eq!(items, Vec::new());
}

#[test]
fn changes_in_the_rendered_projection_are_published() {
    let mut store = AdditionalContextStore::default();
    let first = format!("first{}tail", "x".repeat(40_000));
    let second = format!("second{}tail", "x".repeat(40_000));
    let (_, snapshot) = store.prepare(browser_context(&first));
    store.commit(snapshot);

    let (items, _) = store.prepare(browser_context(&second));
    assert_eq!(items.len(), 1);
}
