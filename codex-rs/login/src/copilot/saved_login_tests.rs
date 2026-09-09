use super::*;
use crate::AuthKeyringBackendKind;
use crate::ConnectionProvider;
use crate::ConnectionStore;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn saved_copilot_login_registers_only_after_completion_and_reuses_credentials() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let root =
        json!({"version": 1, "github_token": "original-token", "machine_id": "a".repeat(64)});
    std::fs::write(
        home.path().join("copilot-auth.json"),
        serde_json::to_vec(&root).unwrap(),
    )
    .unwrap();
    let factory = crate::test_support::transport_default_auth_route_config()
        .http_client_factory()
        .clone();
    let store = ConnectionStore::new(
        home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        crate::AuthRouteConfig::from_http_client_factory(factory.clone()),
    );
    let login = store
        .begin_login(
            ConnectionProvider::Copilot,
            NonEmptyString::new("work github").unwrap(),
        )
        .unwrap();
    let auth = GitHubCopilotAuth::new_with_parts(
        login.home(),
        factory.clone(),
        AuthCredentialsStoreMode::File,
        Arc::new(DefaultKeyringStore),
        OAuthEndpoints::from_base_url(&server.uri()),
        server.uri(),
    );
    assert_eq!(auth.credential().unwrap(), None);
    Mock::given(method("POST")).and(path("/login/device/code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"device_code": "device", "user_code": "ABCD-1234", "verification_uri": format!("{}/login/device", server.uri()), "expires_in": 60, "interval": 1}))).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token": "saved-token", "token_type": "bearer", "scope": "read:user"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", "Bearer saved-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"login": "octocat"})))
        .mount(&server)
        .await;
    let authorization = auth.begin_device_login().await.unwrap();
    assert_eq!(authorization.user_code(), "ABCD-1234");
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(authorization.finish().await.unwrap().as_str(), "octocat");
    let saved = login.finish().unwrap();
    let credential = auth.credential().unwrap().unwrap();
    assert_eq!(credential.token(), "saved-token");
    let manager = crate::AuthManager::shared_from_auth_config(
        crate::auth::AuthConfig {
            codex_home: home.path().to_path_buf(),
            auth_credentials_store_mode: AuthCredentialsStoreMode::File,
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            forced_login_method: None,
            chatgpt_base_url: None,
            forced_chatgpt_workspace_id: None,
            managed_auth_policy: Default::default(),
            auth_route_config: crate::AuthRouteConfig::from_http_client_factory(factory),
        },
        false,
    )
    .await
    .unwrap();
    let prepared = manager.prepare_saved_connection(saved.id()).await.unwrap();
    manager
        .activate_saved_connection(prepared)
        .await
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        manager.copilot_auth().credential().unwrap().unwrap(),
        credential
    );
    let prepared = manager.prepare_saved_connection("copilot").await.unwrap();
    manager
        .activate_saved_connection(prepared)
        .await
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        manager
            .copilot_auth()
            .credential()
            .unwrap()
            .unwrap()
            .token(),
        "original-token"
    );
    assert_eq!(store.list().unwrap().len(), 2);
    let prepared = manager.prepare_saved_connection(saved.id()).await.unwrap();
    manager
        .activate_saved_connection(prepared)
        .await
        .unwrap()
        .commit()
        .unwrap();
    manager.logout().await.unwrap();
    assert_eq!(manager.copilot_auth().credential().unwrap(), None);
    assert_eq!(
        store.list().unwrap(),
        vec![store.resolve("copilot").unwrap()]
    );
}
