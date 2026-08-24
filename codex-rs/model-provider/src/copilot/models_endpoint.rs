use std::sync::Arc;
use std::time::Duration;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpResponse;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::ModelsEndpointClient;
use codex_models_manager::manager::ModelsEndpointFuture;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CoreResult;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::openai_models::default_input_modalities;
use http::HeaderMap;
use http::HeaderValue;
use serde::Deserialize;
use tokio::time::timeout;
use uuid::Uuid;

use super::endpoint::CopilotEndpointManager;
use super::endpoint::EndpointSource;
use super::identity;

const MODELS_REFRESH_TIMEOUT: Duration = Duration::from_secs(10);
const GPT_DEFAULT_CONTEXT_WINDOW: i64 = 400_000;
const COPILOT_MODELS_CACHE_FILE: &str = "copilot_models_cache.json";

pub(super) fn cache_file_name() -> &'static str {
    COPILOT_MODELS_CACHE_FILE
}

#[derive(Debug, Deserialize)]
struct CopilotModelsResponse {
    data: Vec<CopilotModelEntry>,
}

#[derive(Debug, Deserialize)]
struct CopilotModelEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    vendor: Option<String>,
    #[serde(default)]
    model_picker_enabled: bool,
    #[serde(default)]
    supported_endpoints: Vec<String>,
    #[serde(default)]
    capabilities: Option<CopilotCapabilities>,
    #[serde(default)]
    billing: Option<CopilotBilling>,
}

#[derive(Debug, Deserialize)]
struct CopilotCapabilities {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    limits: Option<CopilotLimits>,
    #[serde(default)]
    supports: Option<CopilotSupports>,
}

#[derive(Debug, Deserialize)]
struct CopilotLimits {
    #[serde(default)]
    max_context_window_tokens: Option<i64>,
    #[serde(default)]
    max_prompt_tokens: Option<i64>,
    #[serde(default)]
    max_output_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CopilotSupports {
    #[serde(default)]
    reasoning_effort: Vec<String>,
    #[serde(default)]
    vision: bool,
}

#[derive(Debug, Deserialize)]
struct CopilotBilling {
    #[serde(default)]
    token_prices: Option<CopilotTokenPrices>,
}

#[derive(Debug, Deserialize)]
struct CopilotTokenPrices {
    #[serde(default, rename = "default")]
    default_tier: Option<CopilotTokenPrice>,
    #[serde(default)]
    long_context: Option<CopilotTokenPrice>,
}

#[derive(Debug, Deserialize)]
struct CopilotTokenPrice {
    #[serde(default)]
    context_max: Option<i64>,
}

#[derive(Debug)]
pub(super) struct CopilotModelsEndpoint {
    endpoint_manager: Arc<CopilotEndpointManager>,
}

impl CopilotModelsEndpoint {
    pub(super) fn new(endpoint_manager: Arc<CopilotEndpointManager>) -> Self {
        Self { endpoint_manager }
    }

    async fn list_models(
        &self,
        http_client_factory: HttpClientFactory,
    ) -> CoreResult<(Vec<ModelInfo>, Option<String>)> {
        let mut endpoint = self.endpoint_manager.endpoint().await?;
        let mut url = format!("{}/models", endpoint.base_url);
        let client = http_client_factory
            .build_client(&url, ClientRouteClass::Api)
            .map_err(|error| CodexErr::Fatal(format!("Copilot models client: {error}")))?;
        let mut response = send_models_request(
            &client,
            &url,
            models_headers(&endpoint.headers, endpoint.source),
        )
        .await?;
        if matches!(
            response.status(),
            http::StatusCode::UNAUTHORIZED | http::StatusCode::FORBIDDEN
        ) {
            self.endpoint_manager.reject_generation(endpoint.generation);
            endpoint = self.endpoint_manager.endpoint().await?;
            url = format!("{}/models", endpoint.base_url);
            let client = http_client_factory
                .build_client(&url, ClientRouteClass::Api)
                .map_err(|error| CodexErr::Fatal(format!("Copilot models client: {error}")))?;
            response = send_models_request(
                &client,
                &url,
                models_headers(&endpoint.headers, endpoint.source),
            )
            .await?;
        }

        let status = response.status();
        let etag = response
            .headers()
            .get(http::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(CodexErr::Fatal(format!(
                "Copilot models request returned {status}: {}",
                truncate(&body, 256)
            )));
        }

        let response = response
            .json::<CopilotModelsResponse>()
            .await
            .map_err(|error| CodexErr::Fatal(format!("decode Copilot models: {error}")))?;
        let raw_model_count = response.data.len();
        if raw_model_count == 0 {
            return Err(CodexErr::Fatal(
                "Copilot models response contained no model entries".to_string(),
            ));
        }
        let bundled = bundled_models_response()
            .map(|response| response.models)
            .unwrap_or_default();
        let models = available_models(response.data, &bundled);
        if models.is_empty() {
            return Err(CodexErr::Fatal(format!(
                "Copilot models response contained {raw_model_count} model entries, but none were enabled for Responses-over-WebSocket"
            )));
        }
        Ok((models, etag))
    }
}

impl ModelsEndpointClient for CopilotModelsEndpoint {
    fn has_command_auth(&self) -> bool {
        true
    }

    fn uses_codex_backend(&self) -> ModelsEndpointFuture<'_, bool> {
        Box::pin(async { false })
    }

    fn remote_catalog_is_authoritative(&self) -> bool {
        true
    }

    fn list_models<'a>(
        &'a self,
        _client_version: &'a str,
        http_client_factory: HttpClientFactory,
    ) -> ModelsEndpointFuture<'a, CoreResult<(Vec<ModelInfo>, Option<String>)>> {
        Box::pin(self.list_models(http_client_factory))
    }
}

fn models_headers(source: &HeaderMap, endpoint_source: EndpointSource) -> HeaderMap {
    let mut headers = source.clone();
    identity::prepare_inference_headers(&mut headers, endpoint_source);
    headers.insert(
        http::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(interaction_id) = HeaderValue::from_str(&Uuid::new_v4().to_string()) {
        headers.entry("x-interaction-id").or_insert(interaction_id);
    }
    headers
        .entry("x-initiator")
        .or_insert(HeaderValue::from_static("user"));
    headers
}

async fn send_models_request(
    client: &HttpClient,
    url: &str,
    headers: HeaderMap,
) -> CoreResult<HttpResponse> {
    timeout(
        MODELS_REFRESH_TIMEOUT,
        client.get(url).headers(headers).send(),
    )
    .await
    .map_err(|_| CodexErr::Timeout)?
    .map_err(|error| CodexErr::Fatal(format!("Copilot models request: {error}")))
}

fn is_websocket_responses_model(entry: &CopilotModelEntry) -> bool {
    entry.model_picker_enabled
        && entry
            .capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.kind.as_deref())
            .is_none_or(|kind| kind == "chat")
        && entry
            .supported_endpoints
            .iter()
            .any(|endpoint| endpoint == "ws:/responses")
}

fn available_models(entries: Vec<CopilotModelEntry>, bundled: &[ModelInfo]) -> Vec<ModelInfo> {
    entries
        .into_iter()
        .filter(is_websocket_responses_model)
        .map(|entry| translate_entry(entry, bundled))
        .collect()
}

fn translate_entry(entry: CopilotModelEntry, bundled: &[ModelInfo]) -> ModelInfo {
    let (reported_default_window, reported_maximum_window) = context_windows(&entry);
    let bundled_info = bundled.iter().find(|model| model.slug == entry.id).cloned();
    let is_bundled = bundled_info.is_some();
    let mut info = bundled_info.unwrap_or_else(|| model_info_from_slug(&entry.id));

    info.display_name = entry.name.clone().unwrap_or_else(|| entry.id.clone());
    info.visibility = ModelVisibility::List;
    info.supported_in_api = true;
    info.used_fallback_model_metadata = false;
    info.priority = copilot_priority(&entry.id);
    info.supports_search_tool = false;
    info.web_search_tool_type = Default::default();
    info.supports_reasoning_summary_parameter = false;
    info.supports_image_detail_original = false;
    info.tool_mode = Some(ToolMode::Direct);

    if let Some(supports) = entry
        .capabilities
        .as_ref()
        .and_then(|capabilities| capabilities.supports.as_ref())
    {
        let levels = supports
            .reasoning_effort
            .iter()
            .filter_map(|effort| effort.parse::<ReasoningEffort>().ok())
            .map(|effort| ReasoningEffortPreset {
                effort,
                description: String::new(),
            })
            .collect::<Vec<_>>();
        if !levels.is_empty() {
            let existing_default = info.default_reasoning_level.clone();
            info.default_reasoning_level = existing_default
                .filter(|default| levels.iter().any(|preset| preset.effort == *default))
                .or_else(|| {
                    levels
                        .iter()
                        .find(|preset| preset.effort == ReasoningEffort::Medium)
                        .map(|preset| preset.effort.clone())
                })
                .or_else(|| levels.first().map(|preset| preset.effort.clone()));
            info.supported_reasoning_levels = levels;
        }
        if !supports.vision {
            info.input_modalities = default_input_modalities()
                .into_iter()
                .filter(|modality| *modality == InputModality::Text)
                .collect();
        }
    }

    let maximum_window = reported_maximum_window.or(if is_bundled {
        info.max_context_window
    } else {
        None
    });
    let default_window = reported_default_window
        .or_else(|| curated_default_window(entry.vendor.as_deref(), &entry.id))
        .or(if is_bundled {
            info.context_window
        } else {
            None
        });
    if let Some(maximum_window) = maximum_window {
        info.max_context_window = Some(maximum_window);
        info.context_window = Some(default_window.unwrap_or(maximum_window).min(maximum_window));
    } else if !is_bundled {
        info.context_window = None;
        info.max_context_window = None;
    }
    info
}

fn copilot_priority(model: &str) -> i32 {
    match model {
        "gpt-5.6-sol" => 0,
        "gpt-5.6-terra" => 1,
        "gpt-5.6-luna" => 2,
        "gpt-5.5" => 3,
        "gpt-5.4" => 4,
        "gpt-5.3-codex" => 5,
        "gpt-5-mini" => 6,
        _ => 50,
    }
}

fn context_windows(entry: &CopilotModelEntry) -> (Option<i64>, Option<i64>) {
    let prices = entry
        .billing
        .as_ref()
        .and_then(|billing| billing.token_prices.as_ref());
    let default_window = prices
        .and_then(|prices| prices.default_tier.as_ref())
        .and_then(|price| positive_window(price.context_max));
    let limits = entry
        .capabilities
        .as_ref()
        .and_then(|capabilities| capabilities.limits.as_ref());
    let maximum_window = prices
        .and_then(|prices| prices.long_context.as_ref())
        .and_then(|price| positive_window(price.context_max))
        .or_else(|| limits.and_then(|limits| positive_window(limits.max_prompt_tokens)))
        .or_else(|| {
            limits.and_then(|limits| {
                positive_window(limits.max_context_window_tokens)
                    .zip(positive_window(limits.max_output_tokens))
                    .and_then(|(total, output)| total.checked_sub(output))
                    .filter(|window| *window > 0)
            })
        })
        .or_else(|| limits.and_then(|limits| positive_window(limits.max_context_window_tokens)))
        .or(default_window);
    (default_window, maximum_window)
}

fn curated_default_window(vendor: Option<&str>, slug: &str) -> Option<i64> {
    match vendor {
        Some(vendor) if vendor.eq_ignore_ascii_case("openai") => Some(GPT_DEFAULT_CONTEXT_WINDOW),
        Some(_) => None,
        None if slug.starts_with("gpt-") => Some(GPT_DEFAULT_CONTEXT_WINDOW),
        None => None,
    }
}

fn positive_window(window: Option<i64>) -> Option<i64> {
    window.filter(|window| *window > 0)
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
#[path = "models_endpoint_tests.rs"]
mod tests;
