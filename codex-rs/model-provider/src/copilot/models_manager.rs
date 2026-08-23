use std::sync::Arc;

use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::ModelsManagerFuture;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;
use tokio::sync::TryLockError;

/// Applies Copilot entitlement-aware model fallback to an ordinary model manager.
#[derive(Debug)]
pub(super) struct CopilotModelsManager {
    inner: SharedModelsManager,
}

impl CopilotModelsManager {
    pub(super) fn wrap(inner: SharedModelsManager) -> SharedModelsManager {
        Arc::new(Self { inner })
    }

    fn default_model(models: &[ModelPreset]) -> String {
        models
            .iter()
            .find(|model| model.is_default)
            .or_else(|| models.first())
            .map(|model| model.model.clone())
            .unwrap_or_default()
    }
}

impl ModelsManager for CopilotModelsManager {
    fn list_models(
        &self,
        refresh_strategy: RefreshStrategy,
        http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, Vec<ModelPreset>> {
        self.inner
            .list_models(refresh_strategy, http_client_factory)
    }

    fn raw_model_catalog(
        &self,
        refresh_strategy: RefreshStrategy,
        http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ModelsResponse> {
        self.inner
            .raw_model_catalog(refresh_strategy, http_client_factory)
    }

    fn get_remote_models(&self) -> ModelsManagerFuture<'_, Vec<ModelInfo>> {
        self.inner.get_remote_models()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        self.inner.try_get_remote_models()
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        None
    }

    fn build_available_models(&self, remote_models: Vec<ModelInfo>) -> Vec<ModelPreset> {
        self.inner.build_available_models(remote_models)
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.inner.list_collaboration_modes()
    }

    fn try_list_models(&self) -> Result<Vec<ModelPreset>, TryLockError> {
        self.inner.try_list_models()
    }

    fn get_default_model<'a>(
        &'a self,
        requested: &'a Option<String>,
        allow_provider_model_fallback: bool,
        refresh_strategy: RefreshStrategy,
        http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'a, String> {
        Box::pin(async move {
            let models = self
                .list_models(refresh_strategy, http_client_factory)
                .await;
            if models.is_empty() {
                return requested.clone().unwrap_or_default();
            }
            if let Some(requested) = requested
                && (models.iter().any(|model| model.model == *requested)
                    || !allow_provider_model_fallback)
            {
                return requested.clone();
            }
            Self::default_model(&models)
        })
    }

    fn refresh_if_new_etag(
        &self,
        etag: String,
        http_client_factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ()> {
        self.inner.refresh_if_new_etag(etag, http_client_factory)
    }
}

#[cfg(test)]
#[path = "models_manager_tests.rs"]
mod tests;
