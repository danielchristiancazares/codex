//! Saved logins and reversible activation, isolated from the upstream login API.

mod handoff;
use crate::outgoing_message::ConnectionId;
pub(super) use handoff::ConnectionSwitches;
use handoff::Settlement;

use super::*;
use codex_app_server_protocol::ConnectionLoginChallenge;
use codex_app_server_protocol::ConnectionSelection;
use codex_app_server_protocol::ConnectionText;
use codex_app_server_protocol::SavedConnectionInfo;
use codex_app_server_protocol::SavedConnectionParams;
use codex_app_server_protocol::SavedConnectionProvider;
use codex_app_server_protocol::SavedConnectionResponse;
use codex_login::ConnectionLogin;
use codex_login::ConnectionProvider;
use codex_login::GitHubCopilotAuth;
use codex_login::NonEmptyString;
use codex_login::SavedConnection;
use codex_login::auth::ActivatedConnection;
use codex_protocol::protocol::AgentStatus;

impl AccountRequestProcessor {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "disconnect recovery must finish before another credential handoff begins"
    )]
    pub(crate) async fn connection_switch_closed(&self, owner: ConnectionId) {
        self.connection_switches
            .lock()
            .await
            .disconnected(self, owner)
            .await;
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "credential activation and settlement must stay serialized with connection-close recovery"
    )]
    pub(crate) async fn saved_connection(
        &self,
        owner: ConnectionId,
        params: SavedConnectionParams,
    ) -> Result<SavedConnectionResponse, JSONRPCErrorError> {
        let store = self.auth_manager.connection_store();
        match params {
            SavedConnectionParams::List { active_provider } => {
                let home = self.auth_manager.connection_credential_home();
                let data = store
                    .list()
                    .map_err(connection_error)?
                    .into_iter()
                    .map(|connection| {
                        let selection = if connection.credential_home(&self.config.codex_home)
                            == home
                            && connection.provider().id() == active_provider.as_str()
                        {
                            ConnectionSelection::Retain
                        } else {
                            ConnectionSelection::Switch
                        };
                        connection_info(connection, selection)
                    })
                    .collect::<Result<_, _>>()?;
                Ok(SavedConnectionResponse::Connections { data })
            }
            SavedConnectionParams::Add { provider, name } => {
                self.add_saved_connection(provider, name).await
            }
            SavedConnectionParams::Prepare { id } => {
                let prepared = self
                    .auth_manager
                    .prepare_saved_connection(id.as_str())
                    .await
                    .map_err(connection_error)?;
                let provider_id = prepared.connection().provider().id();
                let provider = self
                    .config
                    .model_providers
                    .get(provider_id)
                    .cloned()
                    .ok_or_else(|| {
                        invalid_request(format!("Provider `{provider_id}` is unavailable."))
                    })?;
                // Each account has its own catalog and auth snapshot, including OpenAI → OpenAI.
                let models = create_model_provider(provider, Some(prepared.auth_manager()))
                    .models_manager_without_cache(
                        if self.config.model_provider_id == provider_id {
                            self.config.model_catalog.clone()
                        } else {
                            None
                        },
                    );
                let data = supported_models(
                    models,
                    /*include_hidden*/ true,
                    self.config.http_client_factory(),
                )
                .await
                .map_err(|error| {
                    internal_error(format!("Could not load this account's models: {error}"))
                })?;
                Ok(SavedConnectionResponse::Prepared { data })
            }
            SavedConnectionParams::Select { id, switch_id } => {
                self.connection_switches
                    .lock()
                    .await
                    .select(self, owner, switch_id.clone(), id)
                    .await?;
                Ok(SavedConnectionResponse::Selected { switch_id })
            }
            SavedConnectionParams::Commit { switch_id } => {
                self.connection_switches
                    .lock()
                    .await
                    .settle(self, owner, switch_id, Settlement::Commit)
                    .await?;
                Ok(SavedConnectionResponse::Settled)
            }
            SavedConnectionParams::Restore { switch_id } => {
                self.connection_switches
                    .lock()
                    .await
                    .settle(self, owner, switch_id, Settlement::Restore)
                    .await?;
                Ok(SavedConnectionResponse::Settled)
            }
        }
    }

    async fn add_saved_connection(
        &self,
        provider: SavedConnectionProvider,
        name: ConnectionText,
    ) -> Result<SavedConnectionResponse, JSONRPCErrorError> {
        self.connection_switches.lock().await.require_ready()?;
        self.cancel_active_login().await;
        let provider = match provider {
            SavedConnectionProvider::Openai => ConnectionProvider::Openai,
            SavedConnectionProvider::Copilot => ConnectionProvider::Copilot,
        };
        let login = self
            .auth_manager
            .connection_store()
            .begin_login(
                provider,
                NonEmptyString::new(name.as_str()).map_err(connection_error)?,
            )
            .map_err(connection_error)?;
        let login_id = Uuid::new_v4();
        let cancel = CancellationToken::new();
        let challenge = match provider {
            ConnectionProvider::Openai => {
                let mut options = self
                    .login_chatgpt_common(
                        /*codex_streamlined_login*/ false,
                        LoginSuccessPage::default(),
                    )
                    .await?;
                options.codex_home = login.home().to_path_buf();
                let server = run_login_server(options).map_err(connection_error)?;
                let challenge = ConnectionLoginChallenge::Browser {
                    url: text(server.auth_url.clone())?,
                };
                let shutdown = server.cancel_handle();
                let mut active = self.active_login.lock().await;
                *active = Some(ActiveLogin::Browser {
                    shutdown_handle: shutdown.clone(),
                    login_id,
                });
                let processor = self.clone();
                tokio::spawn(async move {
                    let result = match tokio::time::timeout(
                        LOGIN_CHATGPT_TIMEOUT,
                        server.block_until_done(),
                    )
                    .await
                    {
                        Ok(result) => result.map(|()| login),
                        Err(_) => {
                            shutdown.shutdown();
                            Err(std::io::Error::other("Login timed out."))
                        }
                    };
                    processor.finish_saved_login(login_id, result).await;
                });
                challenge
            }
            ConnectionProvider::Copilot => {
                let auth = GitHubCopilotAuth::new_in(
                    login.home(),
                    self.config.http_client_factory(),
                    self.config.cli_auth_credentials_store_mode,
                );
                let authorization = auth
                    .begin_device_login()
                    .await
                    .map_err(|error| internal_error(error.to_string()))?;
                let challenge = ConnectionLoginChallenge::DeviceCode {
                    url: text(authorization.verification_url().to_owned())?,
                    code: text(authorization.user_code().to_owned())?,
                };
                let mut active = self.active_login.lock().await;
                *active = Some(ActiveLogin::DeviceCode {
                    cancel: cancel.clone(),
                    login_id,
                });
                let processor = self.clone();
                tokio::spawn(async move {
                    let result = tokio::select! {
                        _ = cancel.cancelled() => Err(std::io::Error::other("Login canceled.")),
                        result = tokio::time::timeout(LOGIN_CHATGPT_TIMEOUT, authorization.finish()) => {
                            match result {
                                Ok(Ok(_)) => Ok(login),
                                Ok(Err(error)) => Err(std::io::Error::other(error)),
                                Err(_) => Err(std::io::Error::other("Login timed out.")),
                            }
                        }
                    };
                    processor.finish_saved_login(login_id, result).await;
                });
                challenge
            }
        };
        Ok(SavedConnectionResponse::LoginStarted {
            login_id: text(login_id.to_string())?,
            challenge,
        })
    }

    async fn finish_saved_login(&self, login_id: Uuid, result: std::io::Result<ConnectionLogin>) {
        // Cancellation and registration affect only the isolated destination.
        let result = {
            let mut active = self.active_login.lock().await;
            if active.as_ref().map(ActiveLogin::login_id) == Some(login_id) {
                *active = None;
                result.and_then(ConnectionLogin::finish)
            } else {
                Err(std::io::Error::other("Login canceled."))
            }
        };
        self.outgoing
            .send_server_notification(ServerNotification::AccountLoginCompleted(
                AccountLoginCompletedNotification {
                    login_id: Some(login_id.to_string()),
                    success: result.is_ok(),
                    error: result.err().map(|error| error.to_string()),
                    onboarding_entrypoint: None,
                },
            ))
            .await;
    }

    async fn notify_connection_changed(&self) {
        self.config_manager.replace_cloud_config_bundle_loader(
            self.auth_manager.clone(),
            self.config.chatgpt_base_url.clone(),
            self.config.http_client_factory(),
        );
        self.config_manager
            .sync_default_client_residency_requirement()
            .await;
        Self::maybe_refresh_plugin_caches_for_current_config(
            &self.config_manager,
            &self.thread_manager,
            self.auth_manager.auth_cached(),
        )
        .await;
        self.outgoing
            .send_server_notification(ServerNotification::AccountUpdated(
                self.current_account_updated_notification(),
            ))
            .await;
    }
}

fn text(value: String) -> Result<ConnectionText, JSONRPCErrorError> {
    ConnectionText::try_from(value).map_err(internal_error)
}

fn connection_error(error: std::io::Error) -> JSONRPCErrorError {
    invalid_request(error.to_string())
}

fn connection_info(
    connection: SavedConnection,
    selection: ConnectionSelection,
) -> Result<SavedConnectionInfo, JSONRPCErrorError> {
    Ok(SavedConnectionInfo {
        id: text(connection.id().to_owned())?,
        name: text(connection.name().to_owned())?,
        selection,
        provider: match connection.provider() {
            ConnectionProvider::Openai => SavedConnectionProvider::Openai,
            ConnectionProvider::Copilot => SavedConnectionProvider::Copilot,
        },
    })
}
