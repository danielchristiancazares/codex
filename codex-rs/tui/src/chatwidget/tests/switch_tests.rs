use super::*;
use crate::connection_switch::SwitchAction;
use crate::connection_switch::SwitchTarget;
use codex_app_server_protocol::ConnectionLoginChallenge;
use codex_app_server_protocol::ConnectionSelection;
use codex_app_server_protocol::SavedConnectionInfo;
use codex_app_server_protocol::SavedConnectionProvider;

pub(super) fn saved_accounts() -> Vec<SavedConnectionInfo> {
    [
        (
            "openai",
            "Personal",
            SavedConnectionProvider::Openai,
            ConnectionSelection::Retain,
        ),
        (
            "work",
            "Work",
            SavedConnectionProvider::Openai,
            ConnectionSelection::Switch,
        ),
        (
            "family",
            "Family",
            SavedConnectionProvider::Openai,
            ConnectionSelection::Switch,
        ),
        (
            "alternate",
            "Alternate",
            SavedConnectionProvider::Openai,
            ConnectionSelection::Switch,
        ),
        (
            "copilot",
            "GitHub Copilot",
            SavedConnectionProvider::Copilot,
            ConnectionSelection::Switch,
        ),
    ]
    .into_iter()
    .map(|(id, name, provider, selection)| SavedConnectionInfo {
        id: id.to_owned().try_into().unwrap(),
        name: name.to_owned().try_into().unwrap(),
        provider,
        selection,
    })
    .collect()
}

#[tokio::test]
async fn switch_picker_selects_another_account_on_the_same_provider() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(Some("gpt-5.2")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.open_provider_popup(saved_accounts());
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_matches!(rx.try_recv(), Ok(AppEvent::SwitchModelProvider(SwitchTarget::SavedAccount(account))) if account == saved_accounts()[1]);
}

#[tokio::test]
async fn switch_inline_nickname_and_add_reach_their_own_flows() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(Some("gpt-5.2")).await;
    chat.handle_switch_args("Work");
    assert_matches!(rx.try_recv(), Ok(AppEvent::ConnectionSwitch(SwitchAction::Named(name))) if name.as_str() == "Work");
    chat.queue_user_message("continue after switching".into());
    assert!(!chat.maybe_send_next_queued_input());
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    chat.handle_switch_args("add");
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::ConnectionSwitch(SwitchAction::AddAccount))
    );
}

#[tokio::test]
async fn switch_add_name_and_device_challenge_snapshots() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(Some("gpt-5.2")).await;
    chat.show_connection_name_prompt(SavedConnectionProvider::Openai);
    assert_chatwidget_snapshot!("switch_account_name", render_bottom_popup(&chat, 80));
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.show_connection_login(
        "test-login".to_owned().try_into().unwrap(),
        ConnectionLoginChallenge::DeviceCode {
            url: "https://github.com/login/device"
                .to_owned()
                .try_into()
                .unwrap(),
            code: "ABCD-1234".to_owned().try_into().unwrap(),
        },
    );
    assert_chatwidget_snapshot!("switch_device_login", render_bottom_popup(&chat, 80));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenUrlInBrowser { url }) if url == "https://github.com/login/device");
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_matches!(rx.try_recv(), Ok(AppEvent::ConnectionSwitch(SwitchAction::CancelLogin(id))) if id.as_str() == "test-login");
}
