//! Owns catalog reuse across provider and credential-scope changes.

use std::path::PathBuf;
use std::sync::Mutex;

use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::SharedModelsManager;

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

    /// The factory runs only when the requested binding changes. The caller
    /// supplies its existing catalog construction policy unchanged.
    pub fn for_provider(
        &self,
        provider: &ModelProviderInfo,
        auth: &AuthManager,
        create: impl FnOnce() -> SharedModelsManager,
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
                manager: create(),
            };
        }
        current.manager.clone()
    }
}
