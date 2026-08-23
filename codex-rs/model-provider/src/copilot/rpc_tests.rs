use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

use super::*;

#[tokio::test]
async fn reads_content_length_frame_after_notification() {
    let (mut writer, reader) = tokio::io::duplex(2048);
    let notification = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "session.event",
        "params": {"type": "session.created"}
    }))
    .expect("encode notification");
    let response = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "id": 7,
        "result": {"ok": true}
    }))
    .expect("encode response");
    for payload in [notification, response] {
        writer
            .write_all(format!("Content-Length: {}\r\n\r\n", payload.len()).as_bytes())
            .await
            .expect("write frame header");
        writer.write_all(&payload).await.expect("write frame body");
    }
    drop(writer);

    let mut reader = BufReader::new(reader);
    let first = read_response(&mut reader).await.expect("read notification");
    let second = read_response(&mut reader).await.expect("read response");

    assert_eq!(first.id, None);
    assert_eq!(second.id, Some(json!(7)));
    assert_eq!(second.result, Some(json!({"ok": true})));
}
