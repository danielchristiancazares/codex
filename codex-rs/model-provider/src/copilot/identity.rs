use http::HeaderMap;
use http::HeaderValue;

pub(super) const CLIENT_APPLICATION: &str = "copilot-cli";
pub(super) const INTEGRATION_ID: &str = "vscode-chat";
pub(super) const INTENT: &str = "conversation-panel";

/// Applies the current Copilot CLI identity while removing REST and legacy editor headers that
/// CAPI does not accept on model requests.
pub(super) fn prepare_inference_headers(headers: &mut HeaderMap) {
    headers.remove("copilot-harness-id");
    headers.remove("x-github-api-version");
    headers.remove("editor-version");
    headers.remove("editor-plugin-version");
    headers.insert(
        "copilot-integration-id",
        HeaderValue::from_static(INTEGRATION_ID),
    );
    headers.insert("openai-intent", HeaderValue::from_static(INTENT));
    headers.insert(
        "x-client-application",
        HeaderValue::from_static(CLIENT_APPLICATION),
    );
}
