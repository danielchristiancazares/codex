//! Unified provider and saved-account selection for the active conversation.

use super::*;
use crate::connection_switch::SwitchAction;
use crate::connection_switch::SwitchTarget;
use crate::connection_switch::provider_id;
use crate::connection_switch::provider_name;
use crate::history_cell::McpInventoryLoadingCell;
use codex_app_server_protocol::ConnectionLoginChallenge;
use codex_app_server_protocol::ConnectionSelection;
use codex_app_server_protocol::ConnectionText;
use codex_app_server_protocol::SavedConnectionInfo;
use codex_app_server_protocol::SavedConnectionProvider;

pub(super) const PROVIDER_SELECTION_VIEW_ID: &str = "provider-selection";
pub(super) const PROVIDER_SWITCH_LOADING_VIEW_ID: &str = "provider-switch-loading";
const CONNECTION_LOGIN_VIEW_ID: &str = "connection-login";

impl ChatWidget {
    pub(crate) fn can_switch_model_provider(&self, thread_id: ThreadId) -> bool {
        self.thread_id == Some(thread_id)
            && !self.active_side_conversation
            && !self.blocks_direct_input
            && !self.config.ephemeral
            && self.unified_exec_processes.is_empty()
            && !self.is_user_turn_pending_or_running()
            && self
                .transcript
                .active_cell
                .as_ref()
                .is_none_or(|cell| !cell.as_any().is::<McpInventoryLoadingCell>())
    }

    pub(crate) fn open_provider_popup(&mut self, accounts: Vec<SavedConnectionInfo>) {
        if !self.is_session_configured() {
            self.add_info_message(
                "Account switching is available after startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }

        let mut items: Vec<_> = accounts
            .iter()
            .map(|account| {
                let target = SwitchTarget::SavedAccount(account.clone());
                SelectionItem {
                    name: account.name.as_str().to_owned(),
                    description: Some(provider_name(account.provider).to_owned()),
                    is_current: matches!(account.selection, ConnectionSelection::Retain),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::SwitchModelProvider(target.clone()));
                    })],
                    dismiss_on_select: true,
                    search_value: Some(format!(
                        "{} {}",
                        account.name.as_str(),
                        provider_name(account.provider)
                    )),
                    ..Default::default()
                }
            })
            .collect();
        for provider in [
            SavedConnectionProvider::Openai,
            SavedConnectionProvider::Copilot,
        ] {
            if !accounts.iter().any(|account| account.provider == provider)
                && self.config.model_provider_id == provider_id(provider)
            {
                let target = SwitchTarget::ConfiguredProvider(provider);
                items.push(SelectionItem {
                    name: format!("{} (configured credentials)", provider_name(provider)),
                    is_current: true,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::SwitchModelProvider(target.clone()))
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                });
            }
            items.push(SelectionItem {
                name: format!("Add {} account…", provider_name(provider)),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::ConnectionSwitch(SwitchAction::NameAccount(
                        provider,
                    )))
                })],
                dismiss_on_select: true,
                ..Default::default()
            });
        }
        let initial_selected_idx = items.iter().position(|item| item.is_current);

        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(PROVIDER_SELECTION_VIEW_ID),
            title: Some("Switch account or provider".to_string()),
            subtitle: Some("Saved logins are reused. Add each account once.".to_string()),
            is_searchable: true,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        });
    }

    pub(super) fn handle_switch_args(&mut self, args: &str) {
        let action = match args {
            "" => SwitchAction::Show,
            "add" => SwitchAction::AddAccount,
            name => match ConnectionText::try_from(name.to_owned()) {
                Ok(name) => SwitchAction::Named(name),
                Err(error) => {
                    self.add_error_message(error.to_owned());
                    return;
                }
            },
        };
        // Resolution is asynchronous; hold queued input before the picker exists.
        self.set_queue_autosend_suppressed(/*suppressed*/ true);
        self.app_event_tx.send(AppEvent::ConnectionSwitch(action));
    }

    pub(crate) fn show_connection_provider_prompt(&mut self) {
        let items = [
            SavedConnectionProvider::Openai,
            SavedConnectionProvider::Copilot,
        ]
        .into_iter()
        .map(|provider| SelectionItem {
            name: provider_name(provider).to_owned(),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::ConnectionSwitch(SwitchAction::NameAccount(
                    provider,
                )))
            })],
            dismiss_on_select: true,
            ..Default::default()
        })
        .collect();
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(PROVIDER_SELECTION_VIEW_ID),
            title: Some("Add account".to_owned()),
            items,
            footer_hint: Some(standard_popup_hint_line()),
            ..Default::default()
        });
    }

    pub(crate) fn show_connection_name_prompt(&mut self, provider: SavedConnectionProvider) {
        let tx = self.app_event_tx.clone();
        self.bottom_pane.show_text_prompt(CustomPromptView::new(
            format!("Name this {} account", provider_name(provider)),
            "Use a short nickname, such as work or personal".to_owned(),
            String::new(),
            /*context_label*/ None,
            Box::new(
                move |name| match ConnectionText::try_from(name.trim().to_owned()) {
                    Ok(name) => tx.send(AppEvent::ConnectionSwitch(SwitchAction::Add {
                        provider,
                        name,
                    })),
                    Err(error) => tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event(error.to_owned()),
                    ))),
                },
            ),
        ));
    }

    pub(crate) fn show_connection_login(
        &mut self,
        login_id: ConnectionText,
        challenge: ConnectionLoginChallenge,
    ) {
        let (url, subtitle) = match challenge {
            ConnectionLoginChallenge::Browser { url } => {
                (url, "Sign in to the account you want to save.".to_owned())
            }
            ConnectionLoginChallenge::DeviceCode { url, code } => (
                url,
                format!("Enter code {} to authorize this account.", code.as_str()),
            ),
        };
        let url = url.as_str().to_owned();
        self.app_event_tx
            .send(AppEvent::OpenUrlInBrowser { url: url.clone() });
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(CONNECTION_LOGIN_VIEW_ID),
            title: Some("Add account".to_owned()),
            subtitle: Some(subtitle),
            items: vec![
                SelectionItem {
                    name: "Open sign-in page".to_owned(),
                    description: Some(url.clone()),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::OpenUrlInBrowser { url: url.clone() })
                    })],
                    ..Default::default()
                },
                SelectionItem {
                    name: "Cancel login".to_owned(),
                    dismiss_on_select: true,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::ConnectionSwitch(SwitchAction::CancelLogin(
                            login_id.clone(),
                        )))
                    })],
                    ..Default::default()
                },
            ],
            footer_hint: Some("Esc hides this view; login continues".dim().into()),
            ..Default::default()
        });
    }

    pub(crate) fn finish_connection_login(&mut self) {
        self.bottom_pane
            .dismiss_view_by_id(CONNECTION_LOGIN_VIEW_ID);
    }

    pub(crate) fn show_provider_switch_loading(&mut self, provider_name: &str) {
        self.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(PROVIDER_SWITCH_LOADING_VIEW_ID),
            title: Some("Switching account or provider".to_string()),
            subtitle: Some(format!("Preparing {provider_name} for this conversation.")),
            items: vec![SelectionItem {
                name: "Loading models and session state...".to_string(),
                is_disabled: true,
                ..Default::default()
            }],
            footer_hint: Some("Press esc to hide; switching continues".dim().into()),
            ..Default::default()
        });
    }

    pub(crate) fn finish_provider_switch_loading(&mut self) {
        self.bottom_pane
            .dismiss_view_by_id(PROVIDER_SWITCH_LOADING_VIEW_ID);
    }
}
