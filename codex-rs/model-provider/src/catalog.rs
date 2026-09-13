//! Constructs provider catalogs from settings independent of core configuration.

use std::sync::Arc;

use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::openai_models::ModelsResponse;

use crate::create_model_provider;

/// Configuration applied when constructing a catalog for an active provider.
#[derive(Clone, Debug, Default)]
pub struct ModelCatalogSettings {
    pub model_catalog: Option<ModelsResponse>,
    pub api_key_model_discovery_enabled: bool,
}

pub fn build_models_manager(
    provider: ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    settings: ModelCatalogSettings,
) -> SharedModelsManager {
    build_catalog(provider, auth_manager, settings, CatalogCache::CredentialScope)
}

/// Lists an inactive provider without applying the active provider's catalog override.
/// Only native OpenAI and Copilot previews reuse the credential scope's disk cache.
pub fn build_preview_models_manager(
    provider: ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
) -> SharedModelsManager {
    let cache = if provider.is_openai() || provider.is_copilot() {
        CatalogCache::CredentialScope
    } else {
        CatalogCache::Disabled
    };
    build_catalog(provider, auth_manager, ModelCatalogSettings::default(), cache)
}

enum CatalogCache {
    CredentialScope,
    Disabled,
}

fn build_catalog(
    provider: ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    settings: ModelCatalogSettings,
    cache: CatalogCache,
) -> SharedModelsManager {
    let credential_home = auth_manager.connection_credential_home();
    let provider = create_model_provider(provider, Some(auth_manager));
    let manager = match cache {
        CatalogCache::CredentialScope => {
            provider.models_manager(credential_home, settings.model_catalog)
        }
        CatalogCache::Disabled => provider.models_manager_without_cache(settings.model_catalog),
    };
    manager.set_api_key_model_discovery_enabled(settings.api_key_model_discovery_enabled);
    manager
}
