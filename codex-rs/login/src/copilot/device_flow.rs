use std::time::Duration;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpResponse;
use codex_http_client::read_response_body_bounded;
use http::HeaderMap;
use http::HeaderValue;
use http::header::ACCEPT;
use http::header::CONTENT_TYPE;
use http::header::USER_AGENT;
use serde::Deserialize;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tokio::time::timeout_at;
use url::Url;

use super::GitHubCopilotAuthError;

const GITHUB_BASE_URL: &str = "https://github.com";
const GITHUB_CLIENT_ID: &str = "Ov23ctDVkRmgkPke0Mmm";
const GITHUB_SCOPES: &str = "read:user";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const MAX_DEVICE_FLOW_DURATION: Duration = Duration::from_secs(15 * 60);
const MAX_POLL_INTERVAL: Duration = Duration::from_secs(60);
const MAX_TOKEN_BYTES: usize = 16 * 1024;
const MAX_OAUTH_RESPONSE_BYTES: usize = 64 * 1024;

pub(super) const COPILOT_USER_AGENT: &str = "GitHubCopilotCLI/1.0.80";

#[derive(Clone, Debug)]
pub(super) struct OAuthEndpoints {
    base_url: String,
    device_code_url: String,
    access_token_url: String,
}

impl OAuthEndpoints {
    pub(super) fn github() -> Self {
        Self::from_base_url(GITHUB_BASE_URL)
    }

    pub(super) fn from_base_url(base_url: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            device_code_url: format!("{base_url}/login/device/code"),
            access_token_url: format!("{base_url}/login/oauth/access_token"),
            base_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

pub(super) struct DeviceCode {
    device_code: String,
    pub(super) user_code: String,
    pub(super) verification_uri: String,
    expires_at: Instant,
    poll_interval: Duration,
}

impl std::fmt::Debug for DeviceCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceCode")
            .field("device_code", &"[REDACTED]")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("expires_at", &self.expires_at)
            .field("poll_interval", &self.poll_interval)
            .finish()
    }
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

pub(super) async fn request_device_code(
    http_client_factory: &HttpClientFactory,
    endpoints: &OAuthEndpoints,
) -> Result<DeviceCode, GitHubCopilotAuthError> {
    let client = http_client_factory
        .build_client_without_redirects_or_request_logging(
            &endpoints.device_code_url,
            ClientRouteClass::Auth,
        )
        .map_err(|error| {
            GitHubCopilotAuthError::oauth(format!("build GitHub device-code client: {error}"))
        })?;
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", GITHUB_CLIENT_ID)
        .append_pair("scope", GITHUB_SCOPES)
        .finish();
    let deadline = Instant::now() + MAX_DEVICE_FLOW_DURATION;
    let response = timeout_at(
        deadline,
        client
            .post(&endpoints.device_code_url)
            .headers(oauth_headers())
            .body(body)
            .send(),
    )
    .await
    .map_err(|_| GitHubCopilotAuthError::oauth("GitHub device-code request timed out"))?
    .map_err(|_| GitHubCopilotAuthError::oauth("request GitHub device code"))?;
    let status = response.status();
    let body = read_oauth_response(response, deadline, "device-code").await?;
    if !status.is_success() {
        return Err(GitHubCopilotAuthError::oauth(format!(
            "GitHub device-code request returned {status}"
        )));
    }
    let response = serde_json::from_slice::<DeviceCodeResponse>(&body)
        .map_err(|_| GitHubCopilotAuthError::oauth("decode GitHub device-code response"))?;
    validate_device_code(response, endpoints)
}

pub(super) async fn complete_device_authorization(
    http_client_factory: &HttpClientFactory,
    endpoints: &OAuthEndpoints,
    device_code: DeviceCode,
) -> Result<String, GitHubCopilotAuthError> {
    let client = http_client_factory
        .build_client_without_redirects_or_request_logging(
            &endpoints.access_token_url,
            ClientRouteClass::Auth,
        )
        .map_err(|error| {
            GitHubCopilotAuthError::oauth(format!("build GitHub OAuth client: {error}"))
        })?;
    let mut poll_interval = device_code.poll_interval;
    loop {
        let remaining = device_code
            .expires_at
            .saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(GitHubCopilotAuthError::oauth(
                "GitHub device authorization expired before completion",
            ));
        }
        sleep_until(Instant::now() + poll_interval.min(remaining)).await;
        if Instant::now() >= device_code.expires_at {
            return Err(GitHubCopilotAuthError::oauth(
                "GitHub device authorization expired before completion",
            ));
        }
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", GITHUB_CLIENT_ID)
            .append_pair("device_code", &device_code.device_code)
            .append_pair("grant_type", DEVICE_GRANT)
            .finish();
        let response = timeout_at(
            device_code.expires_at,
            client
                .post(&endpoints.access_token_url)
                .headers(oauth_headers())
                .body(body)
                .send(),
        )
        .await
        .map_err(|_| {
            GitHubCopilotAuthError::oauth("GitHub device authorization expired before completion")
        })?
        .map_err(|_| GitHubCopilotAuthError::oauth("poll GitHub device authorization"))?;
        let status = response.status();
        let body = read_oauth_response(response, device_code.expires_at, "authorization").await?;
        let response = serde_json::from_slice::<AccessTokenResponse>(&body).map_err(|_| {
            GitHubCopilotAuthError::oauth("decode GitHub device-authorization response")
        })?;
        if let Some(token) = response
            .access_token
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty() && token.len() <= MAX_TOKEN_BYTES)
        {
            return Ok(token);
        }
        match response.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => {
                poll_interval = poll_interval
                    .saturating_add(Duration::from_secs(5))
                    .min(MAX_POLL_INTERVAL);
            }
            Some("expired_token") => {
                return Err(GitHubCopilotAuthError::oauth(
                    "GitHub device authorization expired before completion",
                ));
            }
            Some("access_denied") => {
                return Err(GitHubCopilotAuthError::oauth(
                    "GitHub device authorization was denied",
                ));
            }
            Some(_) => {
                return Err(GitHubCopilotAuthError::oauth(format!(
                    "GitHub device authorization returned {status} with an unsupported error"
                )));
            }
            None => {
                return Err(GitHubCopilotAuthError::oauth(format!(
                    "GitHub device authorization returned {status} without a token or error"
                )));
            }
        }
    }
}

fn validate_device_code(
    response: DeviceCodeResponse,
    endpoints: &OAuthEndpoints,
) -> Result<DeviceCode, GitHubCopilotAuthError> {
    if response.device_code.is_empty() || response.device_code.len() > 256 {
        return Err(GitHubCopilotAuthError::oauth(
            "GitHub device-code response contained an invalid device code",
        ));
    }
    if response.user_code.is_empty() || response.user_code.len() > 32 {
        return Err(GitHubCopilotAuthError::oauth(
            "GitHub device-code response contained an invalid user code",
        ));
    }
    let expected = Url::parse(&endpoints.base_url).map_err(|error| {
        GitHubCopilotAuthError::oauth(format!("parse GitHub OAuth base URL: {error}"))
    })?;
    let verification = Url::parse(&response.verification_uri).map_err(|error| {
        GitHubCopilotAuthError::oauth(format!("parse GitHub verification URL: {error}"))
    })?;
    if verification.scheme() != expected.scheme()
        || verification.host_str() != expected.host_str()
        || verification.port_or_known_default() != expected.port_or_known_default()
        || verification.path() != "/login/device"
    {
        return Err(GitHubCopilotAuthError::oauth(
            "GitHub device-code response contained an unexpected verification URL",
        ));
    }
    let expires_in = Duration::from_secs(response.expires_in).min(MAX_DEVICE_FLOW_DURATION);
    if expires_in.is_zero() {
        return Err(GitHubCopilotAuthError::oauth(
            "GitHub device-code response expired immediately",
        ));
    }
    let poll_interval = Duration::from_secs(response.interval.max(1)).min(MAX_POLL_INTERVAL);
    Ok(DeviceCode {
        device_code: response.device_code,
        user_code: response.user_code,
        verification_uri: response.verification_uri,
        expires_at: Instant::now() + expires_in,
        poll_interval,
    })
}

fn oauth_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    headers.insert(USER_AGENT, HeaderValue::from_static(COPILOT_USER_AGENT));
    headers
}

async fn read_oauth_response(
    response: HttpResponse,
    deadline: Instant,
    operation: &'static str,
) -> Result<Vec<u8>, GitHubCopilotAuthError> {
    timeout_at(
        deadline,
        read_response_body_bounded(response, MAX_OAUTH_RESPONSE_BYTES),
    )
    .await
    .map_err(|_| GitHubCopilotAuthError::oauth(format!("GitHub {operation} response timed out")))?
    .map(Vec::from)
    .map_err(|error| {
        GitHubCopilotAuthError::oauth(format!("read GitHub {operation} response: {error}"))
    })
}

#[cfg(test)]
#[path = "device_flow_tests.rs"]
mod tests;
