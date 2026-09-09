//! UI orchestration for saved accounts. Thread handoff remains in provider_switch.

use super::*;
use crate::connection_switch::LoginCompletion;
use crate::connection_switch::SwitchAction;
use crate::connection_switch::SwitchTarget;
use crate::connection_switch::request;
use codex_app_server_protocol::AccountLoginCompletedNotification;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::CancelLoginAccountResponse;
use codex_app_server_protocol::SavedConnectionParams;
use codex_app_server_protocol::SavedConnectionResponse;

impl App {
    pub(super) async fn handle_connection_switch(
        &mut self,
        app_server: &AppServerSession,
        action: SwitchAction,
    ) {
        if let Err(error) = self.connection_switch_action(app_server, action).await {
            self.chat_widget.add_error_message(error.to_string());
        }
        self.app_event_tx.send(AppEvent::SettingsSelectionSettled);
    }

    async fn connection_switch_action(
        &mut self,
        app_server: &AppServerSession,
        action: SwitchAction,
    ) -> Result<()> {
        if self.app_server_target.uses_remote_workspace() {
            color_eyre::eyre::bail!("Account switching is available in local workspaces.");
        }
        if self.pending_provider_switch.is_some() {
            color_eyre::eyre::bail!("An account switch is already in progress.");
        }
        let handle = app_server.request_handle();
        match action {
            SwitchAction::Show => {
                let accounts = self.saved_connections(&handle).await?;
                self.chat_widget.open_provider_popup(accounts);
            }
            SwitchAction::Named(name) => {
                let accounts = self.saved_connections(&handle).await?;
                let matches: Vec<_> = accounts
                    .into_iter()
                    .filter(|entry| entry.name.as_str().eq_ignore_ascii_case(name.as_str()))
                    .collect();
                match matches.as_slice() {
                    [account] => self.start_model_provider_switch(
                        app_server,
                        SwitchTarget::SavedAccount(account.clone()),
                    ),
                    [] => color_eyre::eyre::bail!(
                        "No saved account named `{}`. Use /switch to choose or add one.",
                        name.as_str()
                    ),
                    _ => color_eyre::eyre::bail!(
                        "Several accounts share that name. Choose one using /switch."
                    ),
                }
            }
            SwitchAction::AddAccount => self.chat_widget.show_connection_provider_prompt(),
            SwitchAction::NameAccount(provider) => {
                self.chat_widget.show_connection_name_prompt(provider)
            }
            SwitchAction::Add { provider, name } => {
                let response =
                    request(&handle, SavedConnectionParams::Add { provider, name }).await?;
                let SavedConnectionResponse::LoginStarted {
                    login_id,
                    challenge,
                } = response
                else {
                    color_eyre::eyre::bail!("The server returned an unexpected login challenge.");
                };
                self.connection_login.started(login_id.clone());
                self.chat_widget.show_connection_login(login_id, challenge);
            }
            SwitchAction::CancelLogin(login_id) => {
                handle
                    .request_typed::<CancelLoginAccountResponse>(
                        ClientRequest::CancelLoginAccount {
                            request_id: codex_app_server_protocol::RequestId::String(format!(
                                "cancel-switch-login-{}",
                                Uuid::new_v4()
                            )),
                            params: CancelLoginAccountParams {
                                login_id: login_id.as_str().to_owned(),
                            },
                        },
                    )
                    .await?;
                self.connection_login.canceled();
                self.chat_widget.finish_connection_login();
                self.app_event_tx.send(AppEvent::SettingsSelectionSettled);
            }
        }
        Ok(())
    }

    async fn saved_connections(
        &self,
        handle: &codex_app_server_client::AppServerRequestHandle,
    ) -> Result<Vec<codex_app_server_protocol::SavedConnectionInfo>> {
        let active_provider = self
            .config
            .model_provider_id
            .clone()
            .try_into()
            .map_err(color_eyre::eyre::Error::msg)?;
        match request(handle, SavedConnectionParams::List { active_provider }).await? {
            SavedConnectionResponse::Connections { data } => Ok(data),
            response @ (SavedConnectionResponse::Prepared { .. }
            | SavedConnectionResponse::Selected { .. }
            | SavedConnectionResponse::LoginStarted { .. }
            | SavedConnectionResponse::Settled) => {
                color_eyre::eyre::bail!("Unexpected account list response: {response:?}")
            }
        }
    }

    pub(super) fn on_connection_login_completed(
        &mut self,
        notification: &AccountLoginCompletedNotification,
    ) {
        match self.connection_login.complete(notification) {
            LoginCompletion::KeepView => {}
            LoginCompletion::RefreshPicker => {
                self.chat_widget.finish_connection_login();
                self.chat_widget.add_info_message(
                    "Account saved. Choose it in /switch when you're ready.".to_owned(),
                    /*hint*/ None,
                );
                self.app_event_tx
                    .send(AppEvent::ConnectionSwitch(SwitchAction::Show));
            }
            LoginCompletion::ReportFailure(error) => {
                self.chat_widget.finish_connection_login();
                self.chat_widget.add_error_message(error.to_string());
                self.app_event_tx.send(AppEvent::SettingsSelectionSettled);
            }
        }
    }
}
