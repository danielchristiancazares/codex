use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::USER_AGENT;

use super::endpoint::EndpointSource;

pub(super) const CLIENT_APPLICATION: &str = "copilot-cli";
pub(super) const INTEGRATION_ID: &str = "vscode-chat";
pub(super) const INTENT: &str = "conversation-panel";
pub(super) const USER_AGENT_VALUE: &str = "GitHubCopilotCLI/1.0.80";
pub(super) const TOKEN_EXCHANGE_USER_AGENT: &str = "GitHubCopilotChat/1.0.0";
pub(super) const EDITOR_VERSION: &str = "Neovim/1.0.0";
pub(super) const EDITOR_PLUGIN_VERSION: &str = "CopilotChat/1.0.0";

/// Applies the identity appropriate to the source of a Copilot endpoint.
pub(super) fn prepare_inference_headers(headers: &mut HeaderMap, source: EndpointSource) {
    headers.remove("copilot-harness-id");
    headers.remove("x-github-api-version");
    headers.insert(
        HeaderName::from_static("copilot-integration-id"),
        HeaderValue::from_static(INTEGRATION_ID),
    );
    headers.insert(
        "x-client-application",
        HeaderValue::from_static(CLIENT_APPLICATION),
    );
    headers.insert(
        HeaderName::from_static("openai-intent"),
        HeaderValue::from_static(INTENT),
    );
    match source {
        EndpointSource::Direct => {
            headers.insert(
                HeaderName::from_static("editor-version"),
                HeaderValue::from_static(EDITOR_VERSION),
            );
            headers.insert(
                HeaderName::from_static("editor-plugin-version"),
                HeaderValue::from_static(EDITOR_PLUGIN_VERSION),
            );
            headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        }
        EndpointSource::Cli => {
            headers.remove("editor-version");
            headers.remove("editor-plugin-version");
        }
    }
}
