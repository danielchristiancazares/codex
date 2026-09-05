use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::read_response_body_bounded;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CoreResult;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use http::HeaderMap;
use http::HeaderValue;
use http::header::ACCEPT;
use http::header::USER_AGENT;
use serde::Deserialize;

use super::endpoint::CopilotEndpointManager;
use super::identity;

const COPILOT_QUOTA_TIMEOUT: Duration = Duration::from_secs(10);
const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2022-11-28";
const MAX_COPILOT_QUOTA_RESPONSE_BYTES: usize = 64 * 1024;
const MONTHLY_WINDOW_MINUTES: i64 = 30 * 24 * 60;
const PREMIUM_INTERACTIONS_QUOTA: &str = "premium_interactions";
const CHAT_QUOTA: &str = "chat";

#[derive(Debug)]
pub(super) struct CopilotQuotaEndpoint {
    endpoint_manager: Arc<CopilotEndpointManager>,
}

impl CopilotQuotaEndpoint {
    pub(super) fn new(endpoint_manager: Arc<CopilotEndpointManager>) -> Self {
        Self { endpoint_manager }
    }

    pub(super) async fn read_rate_limits(
        &self,
        http_client_factory: HttpClientFactory,
    ) -> CoreResult<Vec<RateLimitSnapshot>> {
        let endpoint = self.endpoint_manager.endpoint().await?;
        let url = quota_url(&endpoint.base_url);
        let client = http_client_factory
            .build_client_without_redirects_or_request_logging(&url, ClientRouteClass::Auth)
            .map_err(|error| CodexErr::Fatal(format!("Copilot quota client: {error}")))?;
        let response_body = tokio::time::timeout(COPILOT_QUOTA_TIMEOUT, async {
            let response = client
                .get(&url)
                .headers(quota_headers(&endpoint.headers))
                .send()
                .await
                .map_err(|_| CodexErr::Fatal("Copilot quota request failed".to_string()))?;
            let status = response.status();
            if !status.is_success() {
                return Err(CodexErr::Fatal(format!(
                    "Copilot quota request returned {status}"
                )));
            }
            read_response_body_bounded(response, MAX_COPILOT_QUOTA_RESPONSE_BYTES)
                .await
                .map_err(|error| CodexErr::Fatal(format!("read Copilot quota response: {error}")))
        })
        .await
        .map_err(|_| CodexErr::Timeout)??;
        let response = serde_json::from_slice::<CopilotUserResponse>(&response_body)
            .map_err(|_| CodexErr::Fatal("decode Copilot quota response".to_string()))?;

        rate_limits_from_response(response)
    }
}

#[derive(Debug, Deserialize)]
struct CopilotUserResponse {
    #[serde(default)]
    quota_reset_date: Option<ResetDate>,
    #[serde(default)]
    quota_reset_date_utc: Option<ResetDate>,
    #[serde(default)]
    quota_snapshots: HashMap<String, CopilotQuotaSnapshot>,
}

#[derive(Debug, Deserialize)]
struct CopilotQuotaSnapshot {
    #[serde(default)]
    entitlement: Option<f64>,
    #[serde(default)]
    percent_remaining: Option<f64>,
    #[serde(default)]
    quota_remaining: Option<f64>,
    #[serde(default)]
    quota_reset_at: Option<ResetDate>,
    #[serde(default)]
    remaining: Option<f64>,
    #[serde(default)]
    unlimited: bool,
}

impl CopilotQuotaSnapshot {
    fn remaining_percent(&self) -> Option<f64> {
        if self.unlimited || self.entitlement == Some(-1.0) {
            return None;
        }
        self.percent_remaining
            .filter(|percent| percent.is_finite())
            .map(|percent| percent.clamp(0.0, 100.0))
            .or_else(|| {
                let entitlement = self
                    .entitlement
                    .filter(|entitlement| entitlement.is_finite() && *entitlement > 0.0)?;
                let remaining = self
                    .quota_remaining
                    .or(self.remaining)
                    .filter(|remaining| remaining.is_finite())?;
                Some((remaining / entitlement * 100.0).clamp(0.0, 100.0))
            })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ResetDate {
    Timestamp(i64),
    Text(String),
}

impl ResetDate {
    fn unix_timestamp(&self) -> Option<i64> {
        match self {
            Self::Timestamp(timestamp) => Some(*timestamp),
            Self::Text(value) => value
                .parse::<i64>()
                .ok()
                .or_else(|| utc_midnight_timestamp(value)),
        }
    }
}

fn quota_headers(source: &HeaderMap) -> HeaderMap {
    let mut headers = source.clone();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(identity::USER_AGENT_VALUE),
    );
    headers.insert(
        "x-github-api-version",
        HeaderValue::from_static(GITHUB_API_VERSION),
    );
    headers
}

fn quota_url(copilot_base_url: &str) -> String {
    let loopback = copilot_base_url
        .parse::<http::Uri>()
        .ok()
        .and_then(|uri| uri.host().map(str::to_string))
        .is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_matches(['[', ']'])
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        });
    let base_url = if loopback {
        copilot_base_url.trim_end_matches('/')
    } else {
        GITHUB_API_BASE_URL
    };
    format!("{base_url}/copilot_internal/user")
}

fn rate_limits_from_response(response: CopilotUserResponse) -> CoreResult<Vec<RateLimitSnapshot>> {
    let quota = [PREMIUM_INTERACTIONS_QUOTA, CHAT_QUOTA]
        .into_iter()
        .find_map(|name| response.quota_snapshots.get(name))
        .ok_or_else(|| {
            CodexErr::Fatal(
                "Copilot quota response did not include premium or chat usage".to_string(),
            )
        })?;
    let resets_at = quota
        .quota_reset_at
        .as_ref()
        .or(response.quota_reset_date_utc.as_ref())
        .or(response.quota_reset_date.as_ref())
        .and_then(ResetDate::unix_timestamp);
    let primary = quota
        .remaining_percent()
        .map(|remaining_percent| RateLimitWindow {
            used_percent: 100.0 - remaining_percent,
            window_minutes: Some(MONTHLY_WINDOW_MINUTES),
            resets_at,
        });

    Ok(vec![RateLimitSnapshot {
        limit_id: Some("copilot".to_string()),
        limit_name: Some("Copilot".to_string()),
        normal_model_slug: None,
        primary,
        secondary: None,
        credits: None,
        individual_limit: None,
        spend_control_reached: None,
        plan_type: None,
        rate_limit_reached_type: None,
    }])
}

fn utc_midnight_timestamp(value: &str) -> Option<i64> {
    let date = value.get(..10)?;
    let mut components = date.split('-');
    let year = components.next()?.parse::<i64>().ok()?;
    let month = components.next()?.parse::<i64>().ok()?;
    let day = components.next()?.parse::<i64>().ok()?;
    if components.next().is_some() || !(1..=12).contains(&month) {
        return None;
    }
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        2 if leap_year => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return None,
    };
    if !(1..=days_in_month).contains(&day) {
        return None;
    }

    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days_since_epoch = era * 146_097 + day_of_era - 719_468;
    days_since_epoch.checked_mul(24 * 60 * 60)
}

#[cfg(test)]
#[path = "quota_tests.rs"]
mod tests;
