use super::*;
use crate::AuthDotJson;
use crate::TokenData;
use crate::save_auth;
use base64::Engine;
use codex_protocol::auth::AuthMode;
use pretty_assertions::assert_eq;

fn fixture(user: &str, workspace: &str, refresh: &str) -> AuthDotJson {
    let claims = serde_json::json!({"email": format!("{user}@example.com"), "https://api.openai.com/auth": {
        "chatgpt_user_id": user, "chatgpt_account_id": workspace, "chatgpt_plan_type": "plus"
    }});
    let token = format!(
        "e30.{}.sig",
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&claims).unwrap())
    );
    AuthDotJson {
        auth_mode: Some(AuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token: crate::token_data::parse_chatgpt_jwt_claims(&token).unwrap(),
            access_token: token,
            refresh_token: refresh.into(),
            account_id: Some(workspace.into()),
        }),
        last_refresh: Some(chrono::Utc::now()),
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
        bedrock_access_keys: None,
    }
}

fn store(home: &Path) -> ConnectionStore {
    ConnectionStore::new(
        home.to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        crate::test_support::transport_default_auth_route_config(),
    )
}

async fn runtime(store: &ConnectionStore) -> std::sync::Arc<crate::AuthManager> {
    crate::AuthManager::shared_from_auth_config(
        crate::auth::AuthConfig {
            codex_home: store.home.clone(),
            auth_credentials_store_mode: store.mode,
            keyring_backend_kind: store.keyring,
            forced_login_method: None,
            chatgpt_base_url: None,
            forced_chatgpt_workspace_id: None,
            managed_auth_policy: Default::default(),
            auth_route_config: store.route.clone(),
        },
        false,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn four_accounts_survive_restart_and_switch_back_to_rotated_credentials() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path());
    let original = fixture("first", "shared", "first-refresh");
    save_auth(home.path(), &original, store.mode, store.keyring).unwrap();
    let mut saved = Vec::new();
    for (name, user, workspace) in [
        ("work", "second", "shared"),
        ("personal", "second", "personal"),
        ("fourth", "fourth", "fourth"),
    ] {
        let login = store
            .begin_login(
                ConnectionProvider::Openai,
                NonEmptyString::new(name).unwrap(),
            )
            .unwrap();
        save_auth(
            login.home(),
            &fixture(user, workspace, name),
            store.mode,
            store.keyring,
        )
        .unwrap();
        saved.push(login.finish().unwrap());
    }
    let manager = runtime(&store).await;
    let prepared = manager
        .prepare_saved_connection(saved[0].id())
        .await
        .unwrap();
    manager
        .activate_saved_connection(prepared)
        .await
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        manager
            .auth_cached()
            .unwrap()
            .get_chatgpt_user_id()
            .as_deref(),
        Some("second")
    );
    let rotated = fixture("second", "shared", "rotated");
    save_auth(
        &saved[0].credential_home(home.path()),
        &rotated,
        store.mode,
        store.keyring,
    )
    .unwrap();
    let prepared = manager.prepare_saved_connection("openai").await.unwrap();
    manager
        .activate_saved_connection(prepared)
        .await
        .unwrap()
        .commit()
        .unwrap();
    let prepared = manager
        .prepare_saved_connection(saved[0].id())
        .await
        .unwrap();
    manager
        .activate_saved_connection(prepared)
        .await
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        manager
            .auth_cached()
            .unwrap()
            .get_token_data()
            .unwrap()
            .refresh_token,
        "rotated"
    );
    store.remember(&saved[0]).unwrap();
    assert_eq!(store.list().unwrap().len(), 4);
    let restarted = store.startup().unwrap();
    match restarted {
        StartupConnection::UseSaved(connection) => assert_eq!(connection, saved[0]),
        StartupConnection::UseConfigured => panic!("saved default should survive restart"),
    }
    assert_eq!(
        load_auth_dot_json(home.path(), store.mode, store.keyring)
            .unwrap()
            .unwrap(),
        original
    );
    assert_eq!(
        runtime(&store)
            .await
            .auth_cached()
            .unwrap()
            .get_token_data()
            .unwrap(),
        rotated.tokens.unwrap()
    );
}

#[test]
fn incomplete_login_and_identity_replacement_preserve_registered_accounts() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path());
    let original = fixture("first", "workspace", "refresh");
    save_auth(home.path(), &original, store.mode, store.keyring).unwrap();
    let login = store
        .begin_login(
            ConnectionProvider::Openai,
            NonEmptyString::new("new account").unwrap(),
        )
        .unwrap();
    assert!(login.finish().is_err());
    let existing = store.resolve("openai").unwrap();
    save_auth(
        home.path(),
        &fixture("another", "workspace", "other-refresh"),
        store.mode,
        store.keyring,
    )
    .unwrap();
    assert!(store.validate(&existing).is_err());
    assert_eq!(store.list().unwrap().len(), 1);
}

#[tokio::test]
async fn abandoning_activation_restores_source_and_logout_preserves_other_accounts() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path());
    let source = fixture("source", "shared", "source-refresh");
    save_auth(home.path(), &source, store.mode, store.keyring).unwrap();
    let login = store
        .begin_login(
            ConnectionProvider::Openai,
            NonEmptyString::new("work").unwrap(),
        )
        .unwrap();
    let target = fixture("target", "shared", "target-refresh");
    save_auth(login.home(), &target, store.mode, store.keyring).unwrap();
    let target_connection = login.finish().unwrap();
    let manager = runtime(&store).await;
    let prepared = manager
        .prepare_saved_connection(target_connection.id())
        .await
        .unwrap();
    assert_eq!(
        manager.auth_cached().unwrap().get_token_data().unwrap(),
        source.tokens.clone().unwrap()
    );
    drop(manager.activate_saved_connection(prepared).await.unwrap());
    assert_eq!(
        manager.auth_cached().unwrap().get_token_data().unwrap(),
        source.tokens.clone().unwrap()
    );
    assert!(!home.path().join("connections/selected.json").exists());
    let prepared = manager
        .prepare_saved_connection(target_connection.id())
        .await
        .unwrap();
    manager
        .activate_saved_connection(prepared)
        .await
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        manager.auth_cached().unwrap().get_token_data().unwrap(),
        target.tokens.unwrap()
    );
    manager.logout().await.unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
    store
        .begin_login(
            ConnectionProvider::Openai,
            NonEmptyString::new("work").unwrap(),
        )
        .unwrap();
    assert_eq!(
        runtime(&store)
            .await
            .auth_cached()
            .unwrap()
            .get_token_data()
            .unwrap(),
        source.tokens.clone().unwrap()
    );
    assert_eq!(
        load_auth_dot_json(home.path(), store.mode, store.keyring)
            .unwrap()
            .unwrap(),
        source
    );
}

#[tokio::test]
async fn credential_scopes_serialize_refresh_and_release_authority_on_drop() {
    let home = tempfile::tempdir().unwrap();
    let lease = RefreshLease::acquire(home.path().to_path_buf())
        .await
        .unwrap();
    let contender = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(home.path().join(".credential-refresh.lock"))
        .unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    let other_home = tempfile::tempdir().unwrap();
    let other_account = RefreshLease::acquire(other_home.path().to_path_buf())
        .await
        .unwrap();
    drop(lease);
    contender.try_lock().unwrap();
    drop(other_account);
}

#[tokio::test]
async fn explicit_login_selects_configured_credentials_and_preserves_saved_accounts() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path());
    save_auth(
        home.path(),
        &fixture("source", "workspace", "source-refresh"),
        store.mode,
        store.keyring,
    )
    .unwrap();
    let login = store
        .begin_login(
            ConnectionProvider::Openai,
            NonEmptyString::new("work").unwrap(),
        )
        .unwrap();
    let target = fixture("target", "workspace", "target-refresh");
    save_auth(login.home(), &target, store.mode, store.keyring).unwrap();
    let target_connection = login.finish().unwrap();
    store.remember(&target_connection).unwrap();
    let manager = runtime(&store).await;
    crate::login_with_api_key(home.path(), "new-api-key", store.mode, store.keyring).unwrap();
    assert_eq!(
        manager.auth_cached().unwrap().get_token_data().unwrap(),
        target.tokens.unwrap()
    );
    assert_eq!(
        runtime(&store)
            .await
            .auth_cached()
            .unwrap()
            .get_token()
            .unwrap(),
        "new-api-key"
    );
    manager.reload_configured_login().await.unwrap();
    assert_eq!(
        manager.auth_cached().unwrap().get_token().unwrap(),
        "new-api-key"
    );
    store.validate(&target_connection).unwrap();
    assert_eq!(store.list().unwrap(), vec![target_connection]);
}

#[tokio::test]
async fn cli_logout_removes_selected_copilot_login_and_preserves_root_account() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path());
    let source = fixture("source", "workspace", "source-refresh");
    save_auth(home.path(), &source, store.mode, store.keyring).unwrap();
    let login = store
        .begin_login(
            ConnectionProvider::Copilot,
            NonEmptyString::new("work github").unwrap(),
        )
        .unwrap();
    std::fs::write(
        login.home().join("copilot-auth.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": 1, "github_token": "saved-github-token", "machine_id": "a".repeat(64)
        }))
        .unwrap(),
    )
    .unwrap();
    let target = login.finish().unwrap();
    store.remember(&target).unwrap();
    assert!(
        crate::logout_with_revoke(home.path(), store.mode, store.keyring, &store.route)
            .await
            .unwrap()
    );
    assert_eq!(
        store.list().unwrap(),
        vec![store.resolve("openai").unwrap()]
    );
    assert_eq!(
        runtime(&store)
            .await
            .auth_cached()
            .unwrap()
            .get_token_data()
            .unwrap(),
        source.tokens.unwrap()
    );
    assert_eq!(
        store
            .copilot(&target.credential_home(home.path()))
            .credential()
            .unwrap(),
        None
    );
}

#[test]
fn persisted_connection_identifiers_cannot_escape_their_credential_scope() {
    let home = tempfile::tempdir().unwrap();
    let store = store(home.path());
    save_auth(
        home.path(),
        &fixture("source", "workspace", "refresh"),
        store.mode,
        store.keyring,
    )
    .unwrap();
    let mut record = serde_json::to_value(store.resolve("openai").unwrap()).unwrap();
    for id in ["../auth.json", "C:\\credentials", "saved-../secret", ""] {
        record["id"] = serde_json::json!(id);
        assert!(serde_json::from_value::<SavedConnection>(record.clone()).is_err());
    }
}
