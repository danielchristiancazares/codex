mod auth_provider;
mod credentials;
mod endpoint;
mod executable;
mod identity;
mod models_endpoint;
mod models_manager;
mod native_session;
mod payload;
mod rpc;
mod token_lifetime;

use std::path::PathBuf;
use std::sync::Arc;

use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_api::TransportError;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::cache::ModelsCache;
use codex_models_manager::manager::OpenAiModelsManager;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ModelsResponse;

use self::auth_provider::CopilotAuthProvider;
use self::endpoint::CopilotEndpointManager;
use self::endpoint::shared_endpoint_manager;
use self::models_endpoint::CopilotModelsEndpoint;
use self::models_manager::CopilotModelsManager;
use crate::ProviderAuthScope;
use crate::ResolvedProviderAuth;
use crate::provider::ModelProvider;
use crate::provider::ModelProviderFuture;
use crate::provider::ProviderAccountResult;
use crate::provider::ProviderAccountState;
use crate::provider::ProviderCapabilities;
use crate::provider::ProviderUnauthorizedRecovery;
use crate::provider::RemoteCompactionSupport;
use crate::provider::ResolvedProviderApi;

/// Native GitHub Copilot provider backed by direct credentials or the installed Copilot CLI login.
pub(crate) struct CopilotModelProvider {
    info: ModelProviderInfo,
    endpoint_manager: Arc<CopilotEndpointManager>,
}

impl CopilotModelProvider {
    pub(crate) fn new(info: ModelProviderInfo) -> Self {
        Self {
            info,
            endpoint_manager: shared_endpoint_manager(),
        }
    }

    fn api_provider_from_endpoint(
        &self,
        endpoint: &endpoint::EndpointSnapshot,
    ) -> codex_protocol::error::Result<Provider> {
        let mut info = self.info.clone();
        info.base_url = Some(endpoint.base_url.clone());
        info.to_api_provider(/*auth_mode*/ None)
    }

    fn auth_from_endpoint(&self, endpoint: Arc<endpoint::EndpointSnapshot>) -> SharedAuthProvider {
        Arc::new(CopilotAuthProvider::new(
            endpoint,
            Arc::clone(&self.endpoint_manager),
        ))
    }

    fn models_endpoint(&self) -> Arc<CopilotModelsEndpoint> {
        Arc::new(CopilotModelsEndpoint::new(Arc::clone(
            &self.endpoint_manager,
        )))
    }
}

impl std::fmt::Debug for CopilotModelProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CopilotModelProvider")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl ModelProvider for CopilotModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            namespace_tools: false,
            image_generation: false,
            web_search: false,
            external_web_access: false,
            remote_compaction: RemoteCompactionSupport::Unsupported,
        }
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        None
    }

    fn is_recoverable_auth_error(&self, error: &TransportError) -> bool {
        matches!(
            error,
            TransportError::Http { status, .. }
                if matches!(
                    *status,
                    http::StatusCode::UNAUTHORIZED | http::StatusCode::FORBIDDEN
                )
        )
    }

    fn recover_from_unauthorized(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<ProviderUnauthorizedRecovery>> {
        // The WebSocket transport rejects the exact auth snapshot before recovery reaches this
        // provider. The retry then resolves a fresh endpoint for its concrete request model.
        Box::pin(async { Ok(ProviderUnauthorizedRecovery::Recovered) })
    }

    fn auth(&self) -> ModelProviderFuture<'_, Option<CodexAuth>> {
        Box::pin(async { None })
    }

    fn account_state(&self) -> ProviderAccountResult {
        Ok(ProviderAccountState {
            account: None,
            requires_openai_auth: false,
        })
    }

    fn api_provider(&self) -> ModelProviderFuture<'_, codex_protocol::error::Result<Provider>> {
        Box::pin(async move {
            let endpoint = self.endpoint_manager.endpoint().await?;
            self.api_provider_from_endpoint(&endpoint)
        })
    }

    fn runtime_base_url(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<Option<String>>> {
        Box::pin(async move {
            self.endpoint_manager
                .endpoint()
                .await
                .map(|endpoint| Some(endpoint.base_url.clone()))
        })
    }

    fn api_auth(
        &self,
    ) -> ModelProviderFuture<'_, codex_protocol::error::Result<SharedAuthProvider>> {
        Box::pin(async move {
            let endpoint = self.endpoint_manager.endpoint().await?;
            Ok(self.auth_from_endpoint(endpoint))
        })
    }

    fn resolve_api_for_model<'a>(
        &'a self,
        model: &'a str,
        thread_id: ThreadId,
        _scope: ProviderAuthScope,
    ) -> ModelProviderFuture<'a, codex_protocol::error::Result<ResolvedProviderApi>> {
        Box::pin(async move {
            let endpoint = self
                .endpoint_manager
                .endpoint_for_model(thread_id, model)
                .await?;
            let provider = self.api_provider_from_endpoint(&endpoint)?;
            let auth = ResolvedProviderAuth::new(self.auth_from_endpoint(endpoint));
            Ok(ResolvedProviderApi { provider, auth })
        })
    }

    fn models_manager(
        &self,
        codex_home: PathBuf,
        _config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        CopilotModelsManager::wrap(Arc::new(OpenAiModelsManager::new_with_cache_path(
            codex_home.join(models_endpoint::cache_file_name()),
            self.models_endpoint(),
            /*auth_manager*/ None,
        )))
    }

    fn models_manager_without_cache(
        &self,
        _config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        CopilotModelsManager::wrap(Arc::new(OpenAiModelsManager::new_without_cache(
            self.models_endpoint(),
            /*auth_manager*/ None,
        )))
    }

    fn models_manager_with_cache(
        &self,
        _config_model_catalog: Option<ModelsResponse>,
        cache: Arc<dyn ModelsCache>,
    ) -> SharedModelsManager {
        CopilotModelsManager::wrap(Arc::new(OpenAiModelsManager::new_with_cache(
            cache,
            self.models_endpoint(),
            /*auth_manager*/ None,
        )))
    }
}

#[cfg(test)]
#[path = "copilot_tests.rs"]
mod tests;
