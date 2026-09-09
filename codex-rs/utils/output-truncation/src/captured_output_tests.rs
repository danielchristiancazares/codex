use super::*;
use pretty_assertions::assert_eq;

#[test]
fn recovers_middle_evidence_through_bounded_search_and_range() {
    let mut store = CapturedOutputStore::default();
    let text = format!(
        "{}important evidence{}",
        "before\n".repeat(10_000),
        "\nafter".repeat(10_000)
    );
    let receipt = store.insert(CaptureId::new([1; 16]), text.clone(), 0);
    let search = store
        .read(
            receipt.id(),
            CaptureQuery::Search("important evidence".to_string().try_into().unwrap()),
        )
        .unwrap();
    assert!(search.contains("important evidence"));
    assert!(search.len() <= MAX_QUERY_BYTES);
    let start = text.find("important evidence").unwrap();
    let range = store
        .read(
            receipt.id(),
            CaptureQuery::Range(CaptureRange::new(start, start + 18).unwrap()),
        )
        .unwrap();
    assert!(range.ends_with("important evidence"));
}

#[test]
fn reports_capture_loss_and_expires_oldest_entries() {
    let mut store = CapturedOutputStore::default();
    let receipt = store.insert(
        CaptureId::new([1; 16]),
        "a".repeat(MAX_CAPTURE_BYTES + 100),
        50,
    );
    assert!(receipt.notice().contains("150 bytes omitted"));
    for id in 2..=34 {
        store.insert(CaptureId::new([id; 16]), "recent".to_string(), 0);
    }
    assert!(
        store
            .read(
                receipt.id(),
                CaptureQuery::Range(CaptureRange::new(0, 10).unwrap())
            )
            .is_err()
    );
    assert_eq!(store.captures.len(), MAX_CAPTURES);
    assert_eq!(store.retained_bytes, MAX_CAPTURES * "recent".len());
}

#[test]
fn ranges_respect_utf8_and_session_ownership() {
    let mut store = CapturedOutputStore::default();
    let receipt = store.insert(CaptureId::new([3; 16]), "a🦀b".to_string(), 0);
    let output = store
        .read(
            receipt.id(),
            CaptureQuery::Range(CaptureRange::new(2, 5).unwrap()),
        )
        .unwrap();
    assert!(output.ends_with("Bytes 5..5\n"));
    assert!(
        CapturedOutputStore::default()
            .read(
                receipt.id(),
                CaptureQuery::Range(CaptureRange::new(0, 5).unwrap())
            )
            .is_err()
    );
    assert!(CaptureRange::new(5, 4).is_err());
    assert!(CaptureRange::new(0, MAX_QUERY_BYTES + 1).is_err());
    assert!(NonEmptyString::try_from(String::new()).is_err());
}
