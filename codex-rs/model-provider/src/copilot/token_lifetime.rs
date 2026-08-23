use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;

const MIN_REFRESH_DELAY: Duration = Duration::from_secs(5);

pub(super) struct CredentialLifetime<'a> {
    pub(super) api_key: Option<&'a str>,
    pub(super) session_token: Option<SessionTokenLifetime<'a>>,
    pub(super) fallback: Duration,
}

pub(super) struct SessionTokenLifetime<'a> {
    pub(super) token: &'a str,
    pub(super) expires_at: Option<&'a str>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct JwtLifetime {
    exp: Option<i64>,
    iat: Option<i64>,
}

pub(super) fn refresh_after(input: CredentialLifetime<'_>) -> Duration {
    refresh_after_at(input, Utc::now().timestamp())
}

fn refresh_after_at(input: CredentialLifetime<'_>, now: i64) -> Duration {
    let api_key_lifetime = input
        .api_key
        .and_then(jwt_lifetime)
        .and_then(|lifetime| refresh_delay(lifetime.iat, lifetime.exp, now));
    let session_lifetime = input.session_token.and_then(|session_token| {
        let jwt = jwt_lifetime(session_token.token);
        let expires_at = session_token
            .expires_at
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.timestamp())
            .or_else(|| jwt.as_ref().and_then(|lifetime| lifetime.exp));
        refresh_delay(jwt.and_then(|lifetime| lifetime.iat), expires_at, now)
    });
    api_key_lifetime
        .into_iter()
        .chain(session_lifetime)
        .min()
        .unwrap_or(input.fallback)
}

fn jwt_lifetime(token: &str) -> Option<JwtLifetime> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn refresh_delay(issued_at: Option<i64>, expires_at: Option<i64>, now: i64) -> Option<Duration> {
    let expires_at = expires_at?;
    let remaining = expires_at.saturating_sub(now);
    if remaining <= 0 {
        return Some(MIN_REFRESH_DELAY);
    }
    let issued_at = issued_at.filter(|issued_at| *issued_at < expires_at && *issued_at <= now);
    let refresh_at = issued_at.map_or_else(
        || now.saturating_add(remaining.saturating_mul(4) / 5),
        |issued_at| {
            issued_at.saturating_add(expires_at.saturating_sub(issued_at).saturating_mul(4) / 5)
        },
    );
    let minimum = i64::try_from(MIN_REFRESH_DELAY.as_secs()).unwrap_or(1);
    let delay = refresh_at.saturating_sub(now).max(minimum).min(remaining);
    Some(Duration::from_secs(u64::try_from(delay).unwrap_or(1)))
}

#[cfg(test)]
#[path = "token_lifetime_tests.rs"]
mod tests;
