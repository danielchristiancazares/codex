use codex_model_provider_info::COPILOT_PROVIDER_ID;
use codex_model_provider_info::built_in_model_providers;
use pretty_assertions::assert_eq;

use super::*;

fn provider() -> CopilotModelProvider {
    let info = built_in_model_providers(/*openai_base_url*/ None)
        .remove(COPILOT_PROVIDER_ID)
        .expect("built-in Copilot provider");
    CopilotModelProvider::new(info)
}

fn http_error(status: http::StatusCode) -> TransportError {
    TransportError::Http {
        status,
        url: None,
        headers: None,
        body: None,
    }
}

#[test]
fn copilot_provider_is_websocket_only_and_disables_unsupported_capabilities() {
    let provider = provider();

    assert!(provider.info().supports_websockets);
    assert_eq!(
        provider.capabilities(),
        ProviderCapabilities {
            namespace_tools: false,
            image_generation: false,
            web_search: false,
            external_web_access: false,
            remote_compaction: RemoteCompactionSupport::Unsupported,
        }
    );
}

#[test]
fn copilot_provider_refreshes_endpoint_for_both_auth_rejections() {
    let provider = provider();

    assert!(provider.is_recoverable_auth_error(&http_error(http::StatusCode::UNAUTHORIZED)));
    assert!(provider.is_recoverable_auth_error(&http_error(http::StatusCode::FORBIDDEN)));
    assert!(!provider.is_recoverable_auth_error(&http_error(http::StatusCode::TOO_MANY_REQUESTS)));
}

#[test]
fn copilot_provider_has_no_openai_login_account() {
    let provider = provider();

    assert_eq!(
        provider.account_state(),
        Ok(ProviderAccountState {
            account: None,
            requires_openai_auth: false,
        })
    );
}
