use std::sync::Arc;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::StaticModelsManager;
use codex_protocol::openai_models::ModelVisibility;
use pretty_assertions::assert_eq;

use super::*;

const HTTP_CLIENT_FACTORY: HttpClientFactory =
    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);

#[tokio::test]
async fn unavailable_configured_model_is_preserved_when_fallback_is_disabled() {
    let mut catalog = bundled_models_response().expect("bundled models");
    catalog.models.truncate(1);
    catalog.models[0].visibility = ModelVisibility::List;
    catalog.models[0].priority = 0;
    let manager = CopilotModelsManager::wrap(Arc::new(StaticModelsManager::new(
        /*auth_manager*/ None, catalog,
    )));

    let selected = manager
        .get_default_model(
            &Some("http-only-or-unavailable-model".to_string()),
            /*allow_provider_model_fallback*/ false,
            RefreshStrategy::Offline,
            HTTP_CLIENT_FACTORY,
        )
        .await;

    assert_eq!(selected, "http-only-or-unavailable-model");
}

#[tokio::test]
async fn unavailable_configured_model_uses_catalog_default_when_fallback_is_enabled() {
    let mut catalog = bundled_models_response().expect("bundled models");
    catalog.models.truncate(1);
    let expected = catalog.models[0].slug.clone();
    catalog.models[0].visibility = ModelVisibility::List;
    catalog.models[0].priority = 0;
    let manager = CopilotModelsManager::wrap(Arc::new(StaticModelsManager::new(
        /*auth_manager*/ None, catalog,
    )));

    let selected = manager
        .get_default_model(
            &Some("http-only-or-unavailable-model".to_string()),
            /*allow_provider_model_fallback*/ true,
            RefreshStrategy::Offline,
            HTTP_CLIENT_FACTORY,
        )
        .await;

    assert_eq!(selected, expected);
}
