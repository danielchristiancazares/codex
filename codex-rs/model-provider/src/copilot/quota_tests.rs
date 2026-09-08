use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::CopilotUserResponse;
use super::quota_url;
use super::rate_limits_from_response;

#[test]
fn maps_premium_interactions_to_a_monthly_rate_limit() {
    let response = serde_json::from_value::<CopilotUserResponse>(json!({
        "quota_reset_date_utc": "2026-09-01T00:00:00Z",
        "quota_snapshots": {
            "chat": {
                "entitlement": -1,
                "percent_remaining": 100,
                "unlimited": true
            },
            "premium_interactions": {
                "entitlement": 1500,
                "percent_remaining": 67.5,
                "quota_remaining": 1012.5,
                "unlimited": false
            }
        }
    }))
    .expect("deserialize quota response");

    assert_eq!(
        rate_limits_from_response(response).expect("map premium quota"),
        vec![RateLimitSnapshot {
            limit_id: Some("copilot".to_string()),
            limit_name: Some("Copilot".to_string()),
            normal_model_slug: None,
            primary: Some(RateLimitWindow {
                used_percent: 32.5,
                window_minutes: Some(43_200),
                resets_at: Some(1_788_220_800),
            }),
            secondary: None,
            credits: None,
            individual_limit: None,
            spend_control_reached: None,
            plan_type: None,
            rate_limit_reached_type: None,
        }]
    );
}

#[test]
fn falls_back_to_the_chat_quota_and_derives_remaining_percentage() {
    let response = serde_json::from_value::<CopilotUserResponse>(json!({
        "quota_reset_date": "2026-10-01",
        "quota_snapshots": {
            "chat": {
                "entitlement": 50,
                "remaining": 20,
                "unlimited": false
            }
        }
    }))
    .expect("deserialize quota response");

    assert_eq!(
        rate_limits_from_response(response).expect("map chat quota"),
        vec![RateLimitSnapshot {
            limit_id: Some("copilot".to_string()),
            limit_name: Some("Copilot".to_string()),
            normal_model_slug: None,
            primary: Some(RateLimitWindow {
                used_percent: 60.0,
                window_minutes: Some(43_200),
                resets_at: Some(1_790_812_800),
            }),
            secondary: None,
            credits: None,
            individual_limit: None,
            spend_control_reached: None,
            plan_type: None,
            rate_limit_reached_type: None,
        }]
    );
}

#[test]
fn preserves_unlimited_quota_without_inventing_a_finite_window() {
    let response = serde_json::from_value::<CopilotUserResponse>(json!({
        "quota_snapshots": {
            "premium_interactions": {
                "entitlement": -1,
                "percent_remaining": 100,
                "unlimited": true
            }
        }
    }))
    .expect("deserialize quota response");

    assert_eq!(
        rate_limits_from_response(response).expect("map unlimited quota"),
        vec![RateLimitSnapshot {
            limit_id: Some("copilot".to_string()),
            limit_name: Some("Copilot".to_string()),
            normal_model_slug: None,
            primary: None,
            secondary: None,
            credits: None,
            individual_limit: None,
            spend_control_reached: None,
            plan_type: None,
            rate_limit_reached_type: None,
        }]
    );
}

#[test]
fn quota_url_uses_the_mock_copilot_origin_only_for_loopback() {
    assert_eq!(
        quota_url("http://127.0.0.1:4317"),
        "http://127.0.0.1:4317/copilot_internal/user"
    );
    assert_eq!(
        quota_url("https://api.enterprise.githubcopilot.com"),
        "https://api.github.com/copilot_internal/user"
    );
}
