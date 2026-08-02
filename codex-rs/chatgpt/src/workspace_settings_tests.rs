use super::*;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn hedged_request_uses_second_attempt_when_first_stalls() {
    let attempts = AtomicUsize::new(0);
    let result = first_success_with_hedge(Duration::from_millis(10), || {
        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
        async move {
            if attempt == 0 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok(attempt)
        }
    })
    .await
    .expect("hedged request should succeed");

    assert_eq!(result, 1);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn hedged_request_keeps_fast_request_single_attempt() {
    let attempts = AtomicUsize::new(0);
    let result = first_success_with_hedge(Duration::from_millis(100), || {
        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
        async move { Ok(attempt) }
    })
    .await
    .expect("initial request should succeed");

    assert_eq!(result, 0);
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn hedged_request_waits_for_initial_success_after_hedge_fails() {
    let attempts = AtomicUsize::new(0);
    let result = first_success_with_hedge(Duration::from_millis(5), || {
        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
        async move {
            if attempt == 0 {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(attempt)
            } else {
                anyhow::bail!("hedge failed")
            }
        }
    })
    .await
    .expect("initial request should still succeed");

    assert_eq!(result, 0);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[test]
fn encode_path_segment_leaves_unreserved_ascii_unchanged() {
    assert_eq!(
        encode_path_segment("account-123_ABC.~"),
        "account-123_ABC.~"
    );
}

#[test]
fn encode_path_segment_escapes_path_separators_and_spaces() {
    assert_eq!(
        encode_path_segment("account/123 with space"),
        "account%2F123%20with%20space"
    );
}
