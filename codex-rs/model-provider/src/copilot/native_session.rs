use std::collections::HashMap;
use std::fmt;
use std::process::Stdio;
use std::time::Duration;

use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use serde::Deserialize;
use serde_json::json;
use tokio::process::Child;
use tokio::process::Command;

use super::cli::ResolvedEndpoint;
use super::rpc::JsonRpcClient;
use super::token_lifetime::CredentialLifetime;
use super::token_lifetime::SessionTokenLifetime;

const COPILOT_ENDPOINT_ENV: &str = "COPILOT_ALLOW_GET_PROVIDER_ENDPOINT";
const COPILOT_DISABLE_WEBSOCKET_ENV: &str = "COPILOT_CLI_DISABLE_WEBSOCKET_RESPONSES";
const COPILOT_INTEGRATION_ID_ENV: &str = "GITHUB_COPILOT_INTEGRATION_ID";
const COPILOT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const ENDPOINT_CACHE_TTL: Duration = Duration::from_secs(180);

const PREFERRED_BROKER_MODELS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.3-codex",
    "gpt-5-mini",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    is_authenticated: bool,
    login: Option<String>,
    host: Option<String>,
    status_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsList {
    models: Vec<RpcModel>,
}

#[derive(Debug, Deserialize)]
struct RpcModel {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionCreated {
    session_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderEndpoint {
    #[serde(rename = "type")]
    kind: String,
    wire_api: Option<String>,
    transport: Option<String>,
    base_url: String,
    api_key: Option<String>,
    headers: HashMap<String, String>,
    session_token: Option<ProviderSessionToken>,
}

impl fmt::Debug for ProviderEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderEndpoint")
            .field("kind", &self.kind)
            .field("wire_api", &self.wire_api)
            .field("transport", &self.transport)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field("session_token", &self.session_token)
            .finish()
    }
}

#[derive(Deserialize)]
struct ProviderSessionToken {
    token: String,
    header: String,
    model: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<String>,
}

impl fmt::Debug for ProviderSessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderSessionToken")
            .field("token", &"[REDACTED]")
            .field("header", &self.header)
            .field("model", &self.model)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

pub(super) async fn resolve_endpoint(
    requested_model: Option<&str>,
) -> Result<ResolvedEndpoint, String> {
    let executable = super::executable::resolve()?;
    let mut child = Command::new(&executable)
        .args(["--headless", "--no-auto-update", "--stdio"])
        .env(COPILOT_ENDPOINT_ENV, "true")
        .env(COPILOT_INTEGRATION_ID_ENV, super::identity::INTEGRATION_ID)
        .env_remove(COPILOT_DISABLE_WEBSOCKET_ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("start Copilot CLI `{}`: {error}", executable.display()))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Copilot CLI did not expose stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Copilot CLI did not expose stdout".to_string())?;
    let mut rpc = JsonRpcClient::new(stdout, stdin);

    let result = async {
        rpc.request("connect", json!({})).await?;
        let auth = rpc.request("auth.getStatus", json!({})).await?;
        let auth: AuthStatus = serde_json::from_value(auth)
            .map_err(|error| format!("decode Copilot CLI authentication status: {error}"))?;
        if !auth.is_authenticated {
            let account = auth
                .login
                .as_deref()
                .map(|login| format!(" for `{login}`"))
                .unwrap_or_default();
            let host = auth
                .host
                .as_deref()
                .map(|host| format!(" on {host}"))
                .unwrap_or_default();
            let detail = auth
                .status_message
                .as_deref()
                .map(|message| format!(": {message}"))
                .unwrap_or_default();
            return Err(format!(
                "Copilot CLI is not authenticated{account}{host}{detail}; run `copilot login`"
            ));
        }

        let models = rpc.request("models.list", json!({})).await?;
        let models: ModelsList = serde_json::from_value(models)
            .map_err(|error| format!("decode Copilot CLI model list: {error}"))?;
        let candidates = endpoint_model_candidates(&models.models, requested_model)?;
        let mut last_error = None;
        for model in candidates {
            let expected_bound_model = requested_model.map(|_| model);
            match resolve_model_endpoint(&mut rpc, model, expected_bound_model).await {
                Ok(endpoint) => return Ok(endpoint),
                Err(error) if requested_model.is_none() => last_error = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            "Copilot CLI returned no model with Responses-over-WebSocket support".to_string()
        }))
    }
    .await;

    let _ = rpc.request("runtime.shutdown", json!({})).await;
    drop(rpc);
    finish_child(&mut child).await;
    result
}

async fn resolve_model_endpoint(
    rpc: &mut JsonRpcClient<tokio::process::ChildStdout, tokio::process::ChildStdin>,
    model: &str,
    expected_bound_model: Option<&str>,
) -> Result<ResolvedEndpoint, String> {
    let created = rpc
        .request(
            "session.create",
            json!({
                "model": model,
                "clientName": super::identity::CLIENT_APPLICATION,
                "capi": {"enableWebSocketResponses": true},
                "requestPermission": false,
                "requestUserInput": false,
                "requestElicitation": false,
                "requestExitPlanMode": false,
                "requestAutoModeSwitch": false,
                "hooks": false,
                "includeSubAgentStreamingEvents": true,
                "envValueMode": "direct"
            }),
        )
        .await?;
    let created: SessionCreated = serde_json::from_value(created)
        .map_err(|error| format!("decode Copilot CLI session: {error}"))?;
    let result = async {
        let endpoint = rpc
            .request(
                "session.provider.getEndpoint",
                json!({
                    "sessionId": created.session_id,
                    "modelId": model,
                }),
            )
            .await?;
        let endpoint: ProviderEndpoint = serde_json::from_value(endpoint)
            .map_err(|error| format!("decode Copilot CLI provider endpoint: {error}"))?;
        build_endpoint(endpoint, expected_bound_model)
    }
    .await;
    let _ = rpc
        .request("session.destroy", json!({"sessionId": created.session_id}))
        .await;
    result
}

fn endpoint_model_candidates<'a>(
    models: &'a [RpcModel],
    requested_model: Option<&str>,
) -> Result<Vec<&'a str>, String> {
    if let Some(requested_model) = requested_model {
        return models
            .iter()
            .find(|model| model.id == requested_model)
            .map(|model| vec![model.id.as_str()])
            .ok_or_else(|| {
                format!("Copilot CLI model `{requested_model}` is unavailable for this account")
            });
    }

    let mut candidates = Vec::with_capacity(models.len());
    for preferred in PREFERRED_BROKER_MODELS {
        if let Some(model) = models.iter().find(|model| model.id == *preferred) {
            candidates.push(model.id.as_str());
        }
    }
    for model in models.iter().filter(|model| model.id.starts_with("gpt-")) {
        if !candidates.contains(&model.id.as_str()) {
            candidates.push(model.id.as_str());
        }
    }
    for model in models {
        if !candidates.contains(&model.id.as_str()) {
            candidates.push(model.id.as_str());
        }
    }
    if candidates.is_empty() {
        Err("Copilot CLI returned no models".to_string())
    } else {
        Ok(candidates)
    }
}

fn build_endpoint(
    endpoint: ProviderEndpoint,
    requested_model: Option<&str>,
) -> Result<ResolvedEndpoint, String> {
    if endpoint.kind != "openai" {
        return Err(format!(
            "Copilot CLI selected unsupported provider type `{}`",
            endpoint.kind
        ));
    }
    if endpoint
        .wire_api
        .as_deref()
        .is_some_and(|api| api != "responses")
    {
        return Err(format!(
            "Copilot CLI selected unsupported wire API `{}`",
            endpoint.wire_api.as_deref().unwrap_or_default()
        ));
    }
    if !matches!(endpoint.transport.as_deref(), None | Some("websockets")) {
        return Err(format!(
            "Copilot CLI selected unsupported transport `{}`",
            endpoint.transport.as_deref().unwrap_or_default()
        ));
    }
    if !(endpoint.base_url.starts_with("https://") || endpoint.base_url.starts_with("http://")) {
        return Err("Copilot CLI returned an invalid provider base URL".to_string());
    }

    let refresh_after = super::token_lifetime::refresh_after(CredentialLifetime {
        api_key: endpoint.api_key.as_deref(),
        session_token: endpoint
            .session_token
            .as_ref()
            .map(|session_token| SessionTokenLifetime {
                token: &session_token.token,
                expires_at: session_token.expires_at.as_deref(),
            }),
        fallback: ENDPOINT_CACHE_TTL,
    });
    let mut headers = HeaderMap::new();
    for (name, value) in endpoint.headers {
        insert_endpoint_header(&mut headers, &name, &value)?;
    }
    if let Some(api_key) = endpoint.api_key
        && !headers.contains_key(AUTHORIZATION)
    {
        let mut value = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|error| format!("decode Copilot CLI API credential: {error}"))?;
        value.set_sensitive(true);
        headers.insert(AUTHORIZATION, value);
    }
    let bound_model = endpoint
        .session_token
        .as_ref()
        .and_then(|session_token| session_token.model.clone());
    if let Some(requested_model) = requested_model
        && let Some(bound_model) = bound_model.as_deref()
        && bound_model != requested_model
    {
        return Err(format!(
            "Copilot CLI bound model `{bound_model}` does not match requested model `{requested_model}`"
        ));
    }
    if let Some(session_token) = endpoint.session_token {
        insert_endpoint_header(&mut headers, &session_token.header, &session_token.token)?;
    }

    Ok(ResolvedEndpoint {
        base_url: endpoint.base_url.trim_end_matches('/').to_string(),
        headers,
        bound_model,
        refresh_after,
    })
}

fn insert_endpoint_header(headers: &mut HeaderMap, name: &str, value: &str) -> Result<(), String> {
    let name = HeaderName::from_bytes(name.as_bytes())
        .map_err(|error| format!("decode Copilot CLI header name: {error}"))?;
    let mut value = HeaderValue::from_str(value)
        .map_err(|error| format!("decode Copilot CLI header value: {error}"))?;
    if is_sensitive_header(&name) {
        value.set_sensitive(true);
    }
    headers.insert(name, value);
    Ok(())
}

fn is_sensitive_header(name: &HeaderName) -> bool {
    let name = name.as_str();
    name.eq_ignore_ascii_case("authorization")
        || name.eq_ignore_ascii_case("cookie")
        || name.contains("token")
        || name.contains("secret")
        || name.contains("api-key")
}

async fn finish_child(child: &mut Child) {
    if tokio::time::timeout(COPILOT_SHUTDOWN_TIMEOUT, child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
