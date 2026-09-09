//! The TUI boundary for the fork's saved-connection API.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConnectionSelection;
use codex_app_server_protocol::ConnectionText;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SavedConnectionInfo;
use codex_app_server_protocol::SavedConnectionParams;
use codex_app_server_protocol::SavedConnectionProvider;
use codex_app_server_protocol::SavedConnectionResponse;
use codex_protocol::openai_models::ModelPreset;

#[derive(Debug, Clone)]
pub(crate) enum SwitchTarget {
    ConfiguredProvider(SavedConnectionProvider),
    SavedAccount(SavedConnectionInfo),
}

impl SwitchTarget {
    pub(crate) fn provider_id(&self) -> &str {
        match self {
            Self::ConfiguredProvider(provider) => provider_id(*provider),
            Self::SavedAccount(account) => provider_id(account.provider),
        }
    }

    pub(crate) fn name(&self) -> &str {
        match self {
            Self::ConfiguredProvider(provider) => provider_id(*provider),
            Self::SavedAccount(account) => account.name.as_str(),
        }
    }

    pub(crate) fn selection(&self, provider: &str) -> ConnectionSelection {
        match self {
            Self::ConfiguredProvider(configured) if provider_id(*configured) == provider => {
                ConnectionSelection::Retain
            }
            Self::ConfiguredProvider(_) => ConnectionSelection::Switch,
            Self::SavedAccount(account) => account.selection,
        }
    }

    pub(crate) async fn prepare(
        &self,
        handle: AppServerRequestHandle,
    ) -> color_eyre::Result<Vec<ModelPreset>> {
        match self {
            Self::ConfiguredProvider(provider) => {
                crate::app_server_session::list_models_for_provider_with_request_handle(
                    handle,
                    provider_id(*provider).to_owned(),
                )
                .await
            }
            Self::SavedAccount(account) => {
                match request(
                    &handle,
                    SavedConnectionParams::Prepare {
                        id: account.id.clone(),
                    },
                )
                .await?
                {
                    SavedConnectionResponse::Prepared { data } => Ok(data
                        .into_iter()
                        .map(crate::app_server_session::model_preset_from_api_model)
                        .collect()),
                    response @ (SavedConnectionResponse::Connections { .. }
                    | SavedConnectionResponse::Selected { .. }
                    | SavedConnectionResponse::LoginStarted { .. }
                    | SavedConnectionResponse::Settled) => color_eyre::eyre::bail!(
                        "Unexpected account preparation response: {response:?}"
                    ),
                }
            }
        }
    }

    pub(crate) async fn activate(
        &self,
        handle: AppServerRequestHandle,
    ) -> color_eyre::Result<SwitchHandoff> {
        match self {
            Self::ConfiguredProvider(_) => Ok(SwitchHandoff {
                state: HandoffState::Settled,
            }),
            Self::SavedAccount(account) => {
                let switch_id = uuid::Uuid::new_v4()
                    .to_string()
                    .try_into()
                    .map_err(color_eyre::eyre::Error::msg)?;
                let handoff = SwitchHandoff {
                    state: HandoffState::RestoreOnDrop { handle, switch_id },
                };
                match handoff.select(account.id.clone()).await? {
                    SavedConnectionResponse::Selected { .. } => Ok(handoff),
                    response @ (SavedConnectionResponse::Connections { .. }
                    | SavedConnectionResponse::Prepared { .. }
                    | SavedConnectionResponse::LoginStarted { .. }
                    | SavedConnectionResponse::Settled) => color_eyre::eyre::bail!(
                        "Unexpected account selection response: {response:?}"
                    ),
                }
            }
        }
    }
}

/// Single-use authority to commit or restore the account used for a thread handoff.
pub(crate) struct SwitchHandoff {
    state: HandoffState,
}

enum HandoffState {
    Settled,
    RestoreOnDrop {
        handle: AppServerRequestHandle,
        switch_id: ConnectionText,
    },
}

impl SwitchHandoff {
    async fn select(&self, id: ConnectionText) -> color_eyre::Result<SavedConnectionResponse> {
        match &self.state {
            HandoffState::Settled => color_eyre::eyre::bail!("This handoff is already settled."),
            HandoffState::RestoreOnDrop { handle, switch_id } => {
                request(
                    handle,
                    SavedConnectionParams::Select {
                        id,
                        switch_id: switch_id.clone(),
                    },
                )
                .await
            }
        }
    }

    pub(crate) async fn commit(mut self) -> color_eyre::Result<()> {
        match std::mem::replace(&mut self.state, HandoffState::Settled) {
            HandoffState::Settled => Ok(()),
            HandoffState::RestoreOnDrop { handle, switch_id } => {
                // The replacement is ready. A lost reply must never trigger
                // a rollback behind that live conversation.
                request(&handle, SavedConnectionParams::Commit { switch_id }).await?;
                Ok(())
            }
        }
    }

    pub(crate) async fn restore(mut self) -> color_eyre::Result<()> {
        match &self.state {
            HandoffState::Settled => {}
            HandoffState::RestoreOnDrop { handle, switch_id } => {
                request(
                    handle,
                    SavedConnectionParams::Restore {
                        switch_id: switch_id.clone(),
                    },
                )
                .await?;
            }
        }
        self.state = HandoffState::Settled;
        Ok(())
    }
}

impl Drop for SwitchHandoff {
    fn drop(&mut self) {
        match std::mem::replace(&mut self.state, HandoffState::Settled) {
            HandoffState::Settled => {}
            HandoffState::RestoreOnDrop { handle, switch_id } => {
                tokio::spawn(async move {
                    let _ = request(&handle, SavedConnectionParams::Restore { switch_id }).await;
                });
            }
        }
    }
}

pub(crate) fn provider_id(provider: SavedConnectionProvider) -> &'static str {
    match provider {
        SavedConnectionProvider::Openai => "openai",
        SavedConnectionProvider::Copilot => "copilot",
    }
}

pub(crate) fn provider_name(provider: SavedConnectionProvider) -> &'static str {
    match provider {
        SavedConnectionProvider::Openai => "OpenAI",
        SavedConnectionProvider::Copilot => "GitHub Copilot",
    }
}

pub(crate) async fn request(
    handle: &AppServerRequestHandle,
    params: SavedConnectionParams,
) -> color_eyre::Result<SavedConnectionResponse> {
    handle
        .request_typed(ClientRequest::SavedConnection {
            request_id: RequestId::String(format!("switch-{}", uuid::Uuid::new_v4())),
            params,
        })
        .await
        .map_err(Into::into)
}

#[derive(Debug)]
pub(crate) enum SwitchAction {
    Show,
    AddAccount,
    Named(ConnectionText),
    NameAccount(SavedConnectionProvider),
    Add {
        provider: SavedConnectionProvider,
        name: ConnectionText,
    },
    CancelLogin(ConnectionText),
}

pub(crate) struct LoginTracker {
    state: LoginState,
}

enum LoginState {
    AcceptLogin,
    AwaitCompletion(ConnectionText),
}

impl LoginTracker {
    pub(crate) fn new() -> Self {
        Self {
            state: LoginState::AcceptLogin,
        }
    }

    pub(crate) fn started(&mut self, id: ConnectionText) {
        self.state = LoginState::AwaitCompletion(id);
    }

    pub(crate) fn canceled(&mut self) {
        self.state = LoginState::AcceptLogin;
    }

    pub(crate) fn complete(
        &mut self,
        notification: &codex_app_server_protocol::AccountLoginCompletedNotification,
    ) -> LoginCompletion {
        match &self.state {
            LoginState::AcceptLogin => LoginCompletion::KeepView,
            LoginState::AwaitCompletion(id) => {
                if notification.login_id.as_deref() != Some(id.as_str()) {
                    return LoginCompletion::KeepView;
                }
                self.state = LoginState::AcceptLogin;
                if notification.success {
                    LoginCompletion::RefreshPicker
                } else {
                    LoginCompletion::ReportFailure(color_eyre::eyre::Error::msg(
                        notification
                            .error
                            .as_deref()
                            .filter(|message| !message.trim().is_empty())
                            .unwrap_or("Login failed.")
                            .to_owned(),
                    ))
                }
            }
        }
    }
}

pub(crate) enum LoginCompletion {
    KeepView,
    RefreshPicker,
    ReportFailure(color_eyre::Report),
}
