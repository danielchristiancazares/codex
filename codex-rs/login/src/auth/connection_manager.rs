//! The small adapter between saved credential scopes and the upstream auth manager.

use super::*;
use crate::ConnectionProvider;
use crate::ConnectionStore;
use crate::SavedConnection;
use crate::connections::StartupConnection;

/// Owns the credential location and the provider-specific logout operation.
/// Configured logins retain the ordinary OpenAI logout contract.
#[derive(Clone)]
pub(super) struct CredentialScope {
    home: PathBuf,
    provider: ConnectionProvider,
}

impl CredentialScope {
    pub(super) fn configured(home: PathBuf) -> Self {
        Self {
            home,
            provider: ConnectionProvider::Openai,
        }
    }

    pub(super) fn startup(selection: StartupConnection, home: &Path) -> Self {
        match selection {
            StartupConnection::UseConfigured => Self::configured(home.to_path_buf()),
            StartupConnection::UseSaved(connection) => Self {
                home: connection.credential_home(home),
                provider: connection.provider(),
            },
        }
    }

    pub(super) fn home(&self) -> &Path {
        &self.home
    }

    pub(super) fn provider(&self) -> ConnectionProvider {
        self.provider
    }
}

pub(super) struct CredentialLease {
    _scope: CredentialLeaseScope,
}

enum CredentialLeaseScope {
    UseProcessLock,
    HoldScopeLock {
        _lease: crate::connections::RefreshLease,
    },
}

impl CredentialLease {
    pub(super) async fn acquire(
        home: PathBuf,
        mode: AuthCredentialsStoreMode,
    ) -> std::io::Result<Self> {
        let scope = match mode {
            AuthCredentialsStoreMode::Ephemeral => CredentialLeaseScope::UseProcessLock,
            AuthCredentialsStoreMode::File
            | AuthCredentialsStoreMode::Keyring
            | AuthCredentialsStoreMode::Auto => CredentialLeaseScope::HoldScopeLock {
                _lease: crate::connections::RefreshLease::acquire(home).await?,
            },
        };
        Ok(Self { _scope: scope })
    }
}

/// Proof that a saved login has been loaded and checked against runtime policy.
pub struct PreparedConnection {
    connection: SavedConnection,
    manager: Arc<AuthManager>,
}

/// A selected login with automatic rollback until the conversation handoff commits.
pub struct ActivatedConnection {
    connection: SavedConnection,
    rollback: ConnectionRollback,
}

struct ConnectionRollback {
    manager: Arc<AuthManager>,
    action: DropAction,
}

enum DropAction {
    RetainSelection,
    RestoreSelection {
        scope: CredentialScope,
        cache: Box<CachedAuth>,
    },
}

impl Drop for ConnectionRollback {
    fn drop(&mut self) {
        match &self.action {
            DropAction::RetainSelection => {}
            DropAction::RestoreSelection { scope, cache } => {
                *self
                    .manager
                    .credential_home
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = scope.clone();
                self.manager.set_cached_auth(cache.auth.clone());
                self.manager.mark_credential_revision_changed();
            }
        }
    }
}

impl ActivatedConnection {
    /// The live handoff already succeeded; persistence failure keeps the live selection.
    pub fn commit(mut self) -> std::io::Result<SavedConnection> {
        self.rollback.action = DropAction::RetainSelection;
        self.rollback
            .manager
            .connection_store()
            .remember(&self.connection)?;
        Ok(self.connection)
    }

    pub async fn restore(self) -> std::io::Result<()> {
        let manager = Arc::clone(&self.rollback.manager);
        let _guard = manager
            .refresh_lock
            .acquire()
            .await
            .map_err(std::io::Error::other)?;
        drop(self);
        Ok(())
    }
}

impl PreparedConnection {
    pub fn connection(&self) -> &SavedConnection {
        &self.connection
    }
    pub fn auth_manager(&self) -> Arc<AuthManager> {
        Arc::clone(&self.manager)
    }
}

impl AuthManager {
    pub(super) async fn lock_credential_scope(&self) -> std::io::Result<CredentialLease> {
        CredentialLease::acquire(
            self.connection_credential_home(),
            self.auth_credentials_store_mode,
        )
        .await
    }

    /// Ordinary login writes to the configured store. Select that completed login
    /// without discarding any of the separately registered accounts.
    pub async fn reload_configured_login(&self) -> std::io::Result<()> {
        let _guard = self
            .refresh_lock
            .acquire()
            .await
            .map_err(std::io::Error::other)?;
        *self
            .credential_home
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            CredentialScope::configured(self.codex_home.clone());
        self.mark_credential_revision_changed();
        self.reload().await;
        Ok(())
    }

    pub fn connection_store(&self) -> ConnectionStore {
        ConnectionStore::new(
            self.codex_home.clone(),
            self.auth_credentials_store_mode,
            self.keyring_backend_kind,
            self.auth_route_config.clone(),
        )
    }

    pub fn connection_credential_home(&self) -> PathBuf {
        self.credential_home
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .home
            .clone()
    }

    pub(super) fn connection_logout_provider(&self) -> ConnectionProvider {
        self.credential_home
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .provider()
    }

    pub async fn prepare_saved_connection(&self, id: &str) -> std::io::Result<PreparedConnection> {
        if matches!(
            self.auth_credentials_store_mode,
            AuthCredentialsStoreMode::Ephemeral
        ) {
            return Err(std::io::Error::other(
                "Account switching requires a persistent credential store.",
            ));
        }
        // OPENAI_API_KEY only prefills API-key login; it does not override
        // this manager's saved credentials.
        if self.has_external_auth()
            || self.enable_codex_api_key_env && read_codex_api_key_from_env().is_some()
            || read_codex_access_token_from_env().is_some()
        {
            return Err(std::io::Error::other(
                "Account switching requires locally managed login credentials.",
            ));
        }
        let store = self.connection_store();
        let connection = store.resolve(id)?;
        if connection.provider() == ConnectionProvider::Copilot
            && ["GITHUB_COPILOT_API_TOKEN", "COPILOT_GITHUB_TOKEN"]
                .into_iter()
                .any(|name| read_non_empty_env_var(name).is_some())
        {
            return Err(std::io::Error::other(
                "Account switching requires removing the explicit GitHub Copilot token environment override.",
            ));
        }
        store.validate(&connection)?;
        let manager = Arc::new(
            Self::new_from_auth_config(
                AuthConfig {
                    codex_home: connection.credential_home(&self.codex_home),
                    auth_credentials_store_mode: self.auth_credentials_store_mode,
                    keyring_backend_kind: self.keyring_backend_kind,
                    forced_login_method: self.forced_login_method,
                    chatgpt_base_url: self.chatgpt_base_url.clone(),
                    forced_chatgpt_workspace_id: self.forced_chatgpt_workspace_id(),
                    managed_auth_policy: self.managed_auth_policy.clone(),
                    auth_route_config: self.auth_route_config.clone(),
                },
                /*enable_codex_api_key_env*/ false,
            )
            .await,
        );
        match connection.provider() {
            ConnectionProvider::Openai => {
                manager.auth().await.ok_or_else(|| {
                    std::io::Error::other(
                        "This ChatGPT login is unavailable or restricted by workspace policy.",
                    )
                })?;
            }
            ConnectionProvider::Copilot => {
                manager
                    .copilot_auth()
                    .credential()
                    .map_err(std::io::Error::other)?
                    .ok_or_else(|| std::io::Error::other("Sign in to GitHub Copilot again."))?;
            }
        }
        Ok(PreparedConnection {
            connection,
            manager,
        })
    }

    pub async fn activate_saved_connection(
        self: &Arc<Self>,
        prepared: PreparedConnection,
    ) -> std::io::Result<ActivatedConnection> {
        let _guard = self
            .refresh_lock
            .acquire()
            .await
            .map_err(std::io::Error::other)?;
        self.connection_store().validate(&prepared.connection)?;
        let rollback = ConnectionRollback {
            manager: Arc::clone(self),
            action: DropAction::RestoreSelection {
                scope: self
                    .credential_home
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
                cache: Box::new(
                    self.inner
                        .read()
                        .map_err(|_| std::io::Error::other("Authentication lock is poisoned."))?
                        .clone(),
                ),
            },
        };
        *self
            .credential_home
            .write()
            .map_err(|_| std::io::Error::other("Credential selection lock is poisoned."))? =
            CredentialScope {
                home: prepared.manager.connection_credential_home(),
                provider: prepared.connection.provider(),
            };
        self.set_cached_auth(prepared.manager.auth_cached());
        self.mark_credential_revision_changed();
        Ok(ActivatedConnection {
            connection: prepared.connection,
            rollback,
        })
    }

    pub(super) async fn restore_saved_connection(
        self: &Arc<Self>,
    ) -> Result<(), RefreshTokenError> {
        if self.enable_codex_api_key_env && read_codex_api_key_from_env().is_some()
            || read_codex_access_token_from_env().is_some()
        {
            return Ok(());
        }
        match self.connection_store().startup()? {
            StartupConnection::UseConfigured => Ok(()),
            StartupConnection::UseSaved(connection) => {
                let prepared = Box::pin(self.prepare_saved_connection(connection.id())).await?;
                let mut activated = self.activate_saved_connection(prepared).await?;
                activated.rollback.action = DropAction::RetainSelection;
                Ok(())
            }
        }
    }
}
