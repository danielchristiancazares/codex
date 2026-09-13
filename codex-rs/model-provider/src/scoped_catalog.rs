//! Owns catalog reuse across provider and credential-scope changes.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::SharedModelsManager;

use crate::ModelCatalogSettings;
use crate::build_models_manager;

/// A catalog cache whose binding is checked by its owner before reuse.
pub struct ScopedModelCatalog {
    current: Mutex<BoundCatalog>,
}

struct BoundCatalog {
    provider: ModelProviderInfo,
    credential_home: PathBuf,
    manager: SharedModelsManager,
}

impl ScopedModelCatalog {
    pub fn new(
        provider: ModelProviderInfo,
        auth: &AuthManager,
        manager: SharedModelsManager,
    ) -> Self {
        Self {
            current: Mutex::new(BoundCatalog {
                provider,
                credential_home: auth.connection_credential_home(),
                manager,
            }),
        }
    }

    pub fn current(&self) -> SharedModelsManager {
        self.current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .manager
            .clone()
    }

    /// Constructs a new catalog only when the provider or credential scope changes.
    pub fn for_provider(
        &self,
        provider: &ModelProviderInfo,
        auth: &Arc<AuthManager>,
        settings: ModelCatalogSettings,
    ) -> SharedModelsManager {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let credential_home = auth.connection_credential_home();
        if current.provider != *provider || current.credential_home != credential_home {
            *current = BoundCatalog {
                provider: provider.clone(),
                credential_home,
                manager: build_models_manager(provider.clone(), Arc::clone(auth), settings),
            };
        }
        current.manager.clone()
    }
}

#[cfg(test)]
#[path = "scoped_catalog_tests.rs"]
mod tests;
