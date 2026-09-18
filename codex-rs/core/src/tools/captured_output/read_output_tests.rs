use super::*;
use codex_utils_output_truncation::CapturedOutputStore;
use serde_json::json;

#[test]
fn read_output_validates_identity_and_query_before_access() {
    for input in [
        json!({"output_id": "", "query": "search", "text": "needle"}),
        json!({"output_id": "00000000000000000000000000000001", "query": "search", "text": ""}),
        json!({"output_id": "00000000000000000000000000000001", "query": "range", "start_byte": 10, "end_byte": 1}),
        json!({"output_id": "00000000000000000000000000000001", "query": "range", "start_byte": 0, "end_byte": 3000}),
        json!({"output_id": "00000000000000000000000000000001", "query": "search", "text": "needle", "start_byte": 0}),
    ] {
        assert!(serde_json::from_value::<CaptureRequest>(input).is_err());
    }
}

#[test]
fn captured_output_range_receipt_and_excerpt() {
    let mut store = CapturedOutputStore::default();
    let receipt = store.insert(
        CaptureId::new([1; 16]),
        "first\nmiddle evidence\nlast\n".to_string(),
        0,
    );
    let output = store
        .read(
            receipt.id(),
            CaptureQuery::Range(CaptureRange::new(6, 22).unwrap()),
        )
        .unwrap();
    insta::assert_snapshot!(output, @r"
    Captured output 01010101010101010101010101010101. Retained 27 bytes; 0 bytes omitted. Query with read_output. Session-local, bounded retention.
    Bytes 6..22
    middle evidence
    ");
}
