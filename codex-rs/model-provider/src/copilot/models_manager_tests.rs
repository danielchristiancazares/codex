use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

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

#[derive(Clone, Debug, Eq, PartialEq)]
enum RecordedListCall {
    BestEffort(RefreshStrategy),
    Fallible(RefreshStrategy),
}

#[derive(Debug)]
struct SequenceModelsManager {
    best_effort: Mutex<VecDeque<Vec<ModelPreset>>>,
    fallible: Mutex<VecDeque<CoreResult<Vec<ModelPreset>>>>,
    calls: Mutex<Vec<RecordedListCall>>,
}

impl SequenceModelsManager {
    fn new(
        best_effort: Vec<Vec<ModelPreset>>,
        fallible: Vec<CoreResult<Vec<ModelPreset>>>,
    ) -> Self {
        Self {
            best_effort: Mutex::new(best_effort.into()),
            fallible: Mutex::new(fallible.into()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<RecordedListCall> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ModelsManager for SequenceModelsManager {
    fn list_models(
        &self,
        refresh_strategy: RefreshStrategy,
        _http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, Vec<ModelPreset>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(RecordedListCall::BestEffort(refresh_strategy));
            self.best_effort
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop_front()
                .expect("best-effort model response")
        })
    }

    fn list_models_with_refresh_errors(
        &self,
        refresh_strategy: RefreshStrategy,
        _http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, CoreResult<Vec<ModelPreset>>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(RecordedListCall::Fallible(refresh_strategy));
            self.fallible
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop_front()
                .expect("fallible model response")
        })
    }

    fn raw_model_catalog(
        &self,
        _refresh_strategy: RefreshStrategy,
        _http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ModelsResponse> {
        Box::pin(async { ModelsResponse { models: Vec::new() } })
    }

    fn get_remote_models(&self) -> ModelsManagerFuture<'_, Vec<ModelInfo>> {
        Box::pin(async { Vec::new() })
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        Ok(Vec::new())
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        None
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        Vec::new()
    }

    fn refresh_if_new_etag(
        &self,
        _etag: String,
        _http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ()> {
        Box::pin(async {})
    }
}

fn one_model_preset() -> Vec<ModelPreset> {
    let mut catalog = bundled_models_response().expect("bundled models");
    vec![catalog.models.remove(0).into()]
}

#[tokio::test]
async fn online_if_uncached_retries_empty_best_effort_catalog_online() {
    let expected = one_model_preset();
    let inner = Arc::new(SequenceModelsManager::new(
        vec![Vec::new(), expected.clone()],
        Vec::new(),
    ));
    let manager = CopilotModelsManager::wrap(inner.clone());

    let actual = manager
        .list_models(RefreshStrategy::OnlineIfUncached, HTTP_CLIENT_FACTORY)
        .await;

    assert_eq!(actual, expected);
    assert_eq!(
        inner.calls(),
        vec![
            RecordedListCall::BestEffort(RefreshStrategy::OnlineIfUncached),
            RecordedListCall::BestEffort(RefreshStrategy::Online),
        ]
    );
}

#[tokio::test]
async fn online_if_uncached_retries_empty_fallible_catalog_online() {
    let expected = one_model_preset();
    let inner = Arc::new(SequenceModelsManager::new(
        Vec::new(),
        vec![Ok(Vec::new()), Ok(expected.clone())],
    ));
    let manager = CopilotModelsManager::wrap(inner.clone());

    let actual = manager
        .list_models_with_refresh_errors(RefreshStrategy::OnlineIfUncached, HTTP_CLIENT_FACTORY)
        .await
        .expect("online retry succeeds");

    assert_eq!(actual, expected);
    assert_eq!(
        inner.calls(),
        vec![
            RecordedListCall::Fallible(RefreshStrategy::OnlineIfUncached),
            RecordedListCall::Fallible(RefreshStrategy::Online),
        ]
    );
}

#[tokio::test]
async fn offline_empty_catalog_remains_offline_for_both_listing_paths() {
    let inner = Arc::new(SequenceModelsManager::new(
        vec![Vec::new()],
        vec![Ok(Vec::new())],
    ));
    let manager = CopilotModelsManager::wrap(inner.clone());

    let best_effort = manager
        .list_models(RefreshStrategy::Offline, HTTP_CLIENT_FACTORY)
        .await;
    let fallible = manager
        .list_models_with_refresh_errors(RefreshStrategy::Offline, HTTP_CLIENT_FACTORY)
        .await
        .expect("offline listing succeeds");

    assert_eq!(best_effort, Vec::<ModelPreset>::new());
    assert_eq!(fallible, Vec::<ModelPreset>::new());
    assert_eq!(
        inner.calls(),
        vec![
            RecordedListCall::BestEffort(RefreshStrategy::Offline),
            RecordedListCall::Fallible(RefreshStrategy::Offline),
        ]
    );
}

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
