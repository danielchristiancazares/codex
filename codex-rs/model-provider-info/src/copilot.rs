use super::ModelProviderInfo;
use super::WireApi;

const COPILOT_PROVIDER_NAME: &str = "GitHub Copilot";
pub const COPILOT_PROVIDER_ID: &str = "copilot";
pub const COPILOT_BASE_URL: &str = "https://api.githubcopilot.com";

pub(super) fn provider() -> ModelProviderInfo {
    ModelProviderInfo {
        name: COPILOT_PROVIDER_NAME.into(),
        base_url: Some(COPILOT_BASE_URL.into()),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: true,
        supports_standalone_web_search: false,
    }
}

pub(super) fn is_provider(provider: &ModelProviderInfo) -> bool {
    provider.name == COPILOT_PROVIDER_NAME && provider.base_url.as_deref() == Some(COPILOT_BASE_URL)
}
