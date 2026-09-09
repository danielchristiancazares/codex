use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::*;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::ConnectionProvider;
use codex_login::ConnectionStore;
use codex_login::NonEmptyString;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn manage(
    server: &mut TestAppServer,
    params: SavedConnectionParams,
) -> Result<SavedConnectionResponse> {
    server
        .request(|request_id| ClientRequest::SavedConnection { request_id, params })
        .await
}

async fn account(server: &mut TestAppServer) -> Result<GetAccountResponse> {
    server
        .request(|request_id| ClientRequest::GetAccount {
            request_id,
            params: GetAccountParams {
                refresh_token: false,
            },
        })
        .await
}

async fn select(server: &mut TestAppServer, id: &str) -> Result<ConnectionText> {
    match manage(
        server,
        SavedConnectionParams::Select {
            id: ConnectionText::try_from(id.to_owned()).map_err(anyhow::Error::msg)?,
            switch_id: ConnectionText::try_from(uuid::Uuid::new_v4().to_string())
                .map_err(anyhow::Error::msg)?,
        },
    )
    .await?
    {
        SavedConnectionResponse::Selected { switch_id } => Ok(switch_id),
        response => anyhow::bail!("Unexpected response: {response:?}"),
    }
}

fn fixture(user: &str) -> ChatGptAuthFixture {
    ChatGptAuthFixture::new(format!("access-{user}"))
        .refresh_token(format!("refresh-{user}"))
        .chatgpt_user_id(user)
        .chatgpt_account_id("shared-workspace")
        .account_id("shared-workspace")
        .email(format!("{user}@example.com"))
        .plan_type("plus")
}

fn write_config(home: &std::path::Path, server: &MockServer) -> Result<()> {
    let base_url = server.uri();
    std::fs::write(
        home.join("config.toml"),
        format!(
            r#"
model = "gpt-5.2"
model_provider = "openai"
cli_auth_credentials_store = "file"
approval_policy = "never"
sandbox_mode = "danger-full-access"
chatgpt_base_url = "{base_url}"
openai_base_url = "{base_url}/v1"
[features]
shell_snapshot = false
"#
        ),
    )?;
    Ok(())
}

async fn turn(server: &mut TestAppServer, thread_id: &str, text: &str) -> Result<()> {
    let _: TurnStartResponse = server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread_id.to_owned(),
                input: vec![UserInput::Text {
                    text: text.to_owned(),
                    text_elements: vec![],
                }],
                ..Default::default()
            },
        })
        .await?;
    let notification = tokio::time::timeout(
        Duration::from_secs(30),
        server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let ServerNotification::TurnCompleted(completed) = notification.try_into()? else {
        anyhow::bail!("expected turn completion");
    };
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    Ok(())
}

#[tokio::test]
async fn saved_connections_prepare_without_activation_restore_and_fork_with_target_credentials()
-> Result<()> {
    let backend = MockServer::start().await;
    let home = tempfile::tempdir()?;
    write_config(home.path(), &backend)?;
    write_chatgpt_auth(
        home.path(),
        fixture("source"),
        AuthCredentialsStoreMode::File,
    )?;
    let original_auth = std::fs::read(home.path().join("auth.json"))?;
    let store = ConnectionStore::new(
        home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        codex_login::AuthRouteConfig::from_http_client_factory(
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
        ),
    );
    let mut saved = Vec::new();
    for user in ["work", "family", "alternate"] {
        let login = store.begin_login(ConnectionProvider::Openai, NonEmptyString::new(user)?)?;
        write_chatgpt_auth(login.home(), fixture(user), AuthCredentialsStoreMode::File)?;
        saved.push(login.finish()?);
    }
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .mount(&backend)
        .await;
    let mut work_model = codex_models_manager::bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-5.2")
        .unwrap();
    work_model.display_name = "Work account model".to_owned();
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("authorization", "Bearer access-work"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [work_model]})))
        .with_priority(/*p*/ 1)
        .expect(1..)
        .mount(&backend)
        .await;
    // An ambient API key must leave saved-account discovery and selection intact.
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", Some("unused-ambient-key"))])
        .build_initialized()
        .await?;
    let source_account = account(&mut server).await?;
    let data = manage(
        &mut server,
        SavedConnectionParams::List {
            active_provider: "openai".to_owned().try_into().unwrap(),
        },
    )
    .await?;
    assert!(matches!(data, SavedConnectionResponse::Connections { data } if data.len() == 4));
    let prepared = manage(
        &mut server,
        SavedConnectionParams::Prepare {
            id: saved[0].id().to_owned().try_into().unwrap(),
        },
    )
    .await?;
    assert!(
        matches!(prepared, SavedConnectionResponse::Prepared { data } if data.iter().any(|model| model.display_name == "Work account model"))
    );
    assert_eq!(account(&mut server).await?, source_account);
    let invalid_request = server
        .send_request(
            "savedConnection/manage",
            Some(serde_json::to_value(SavedConnectionParams::Select {
                id: "missing".to_owned().try_into().unwrap(),
                switch_id: "missing-selection".to_owned().try_into().unwrap(),
            })?),
        )
        .await?;
    let rejected = server
        .read_stream_until_error_message(RequestId::Integer(invalid_request))
        .await?;
    assert_eq!(
        rejected.error,
        JSONRPCErrorError {
            code: -32600,
            message: "The selected connection is no longer available.".to_owned(),
            data: None,
        }
    );
    let switch_id = select(&mut server, saved[0].id()).await?;
    manage(
        &mut server,
        SavedConnectionParams::Select {
            id: saved[0].id().to_owned().try_into().unwrap(),
            switch_id: switch_id.clone(),
        },
    )
    .await?;
    manage(
        &mut server,
        SavedConnectionParams::Restore {
            switch_id: switch_id.clone(),
        },
    )
    .await?;
    manage(&mut server, SavedConnectionParams::Restore { switch_id }).await?;
    assert_eq!(account(&mut server).await?, source_account);

    // Keep model discovery on HTTP while exercising inference over the production transport.
    let responses_backend = responses::start_websocket_server(vec![
        vec![
            vec![
                responses::ev_response_created("source-warmup"),
                responses::ev_completed("source-warmup"),
            ],
            vec![
                responses::ev_response_created("source"),
                responses::ev_completed("source"),
            ],
        ],
        vec![
            vec![
                responses::ev_response_created("target-warmup"),
                responses::ev_completed("target-warmup"),
            ],
            vec![
                responses::ev_response_created("target"),
                responses::ev_completed("target"),
            ],
        ],
    ])
    .await;
    let inference_config = HashMap::from([(
        "openai_base_url".to_owned(),
        json!(format!(
            "{}/v1",
            responses_backend.uri().replacen("ws://", "http://", 1)
        )),
    )]);
    let request_id = server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            config: Some(inference_config.clone()),
            ..Default::default()
        })
        .await?;
    let started: ThreadStartResponse = server.read_response(request_id).await?;
    turn(
        &mut server,
        &started.thread.id,
        "source-context-to-preserve",
    )
    .await?;
    assert_eq!(
        responses_backend
            .single_handshake()
            .header("authorization")
            .as_deref(),
        Some("Bearer access-source")
    );

    let switch_id = select(&mut server, saved[0].id()).await?;
    let catalog: ModelListResponse = server
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams::default(),
        })
        .await?;
    assert!(
        catalog
            .data
            .iter()
            .any(|model| model.display_name == "Work account model")
    );
    let fork: ThreadForkResponse = server
        .request(|request_id| ClientRequest::ThreadFork {
            request_id,
            params: ThreadForkParams {
                thread_id: started.thread.id.clone(),
                config: Some(inference_config),
                ..Default::default()
            },
        })
        .await?;
    manage(
        &mut server,
        SavedConnectionParams::Commit {
            switch_id: switch_id.clone(),
        },
    )
    .await?;
    manage(&mut server, SavedConnectionParams::Commit { switch_id }).await?;
    let target_account = account(&mut server).await?;
    assert_ne!(target_account, source_account);
    turn(&mut server, &fork.thread.id, "continue-on-work-account").await?;
    assert_eq!(
        responses_backend
            .handshakes()
            .iter()
            .map(|handshake| (
                handshake.header("authorization"),
                handshake.header("chatgpt-account-id"),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                Some("Bearer access-source".to_owned()),
                Some("shared-workspace".to_owned()),
            ),
            (
                Some("Bearer access-work".to_owned()),
                Some("shared-workspace".to_owned()),
            ),
        ]
    );
    let connections = responses_backend.connections();
    let [_, target] = connections.as_slice() else {
        anyhow::bail!("expected source and target connections");
    };
    let [_, outbound] = target.as_slice() else {
        anyhow::bail!("expected target warmup and inference requests");
    };
    let outbound = outbound.body_json();
    assert_eq!(outbound["previous_response_id"], json!("target-warmup"));
    assert!(
        outbound["input"]
            .to_string()
            .contains("source-context-to-preserve")
    );
    assert!(
        outbound["input"]
            .to_string()
            .contains("continue-on-work-account")
    );
    responses_backend.shutdown().await;
    assert_eq!(std::fs::read(home.path().join("auth.json"))?, original_auth);
    drop(server);
    let mut restarted = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", Some("unused-ambient-key"))])
        .build_initialized()
        .await?;
    assert_eq!(account(&mut restarted).await?, target_account);
    let _: LoginAccountResponse = restarted
        .request(|request_id| ClientRequest::LoginAccount {
            request_id,
            params: LoginAccountParams::ApiKey {
                api_key: "configured-api-key".to_owned(),
            },
        })
        .await?;
    assert!(matches!(
        account(&mut restarted).await?.account,
        Some(Account::ApiKey {})
    ));
    assert!(!home.path().join("connections/selected.json").exists());
    let switch_id = select(&mut restarted, saved[0].id()).await?;
    manage(&mut restarted, SavedConnectionParams::Commit { switch_id }).await?;
    assert_eq!(account(&mut restarted).await?, target_account);
    Ok(())
}

#[tokio::test]
async fn saved_connection_refresh_is_serialized_between_app_server_processes() -> Result<()> {
    let backend = MockServer::start().await;
    let home = tempfile::tempdir()?;
    write_config(home.path(), &backend)?;
    write_chatgpt_auth(
        home.path(),
        fixture("source"),
        AuthCredentialsStoreMode::File,
    )?;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(400))
                .set_body_json(json!({
                    "access_token": "rotated-access", "refresh_token": "rotated-refresh"
                })),
        )
        .expect(1)
        .mount(&backend)
        .await;
    let endpoint = format!("{}/oauth/token", backend.uri());
    let mut first = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[(
            codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
            Some(&endpoint),
        )])
        .build_initialized()
        .await?;
    let mut second = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[(
            codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
            Some(&endpoint),
        )])
        .build_initialized()
        .await?;
    let request = |request_id| ClientRequest::GetAccount {
        request_id,
        params: GetAccountParams {
            refresh_token: true,
        },
    };
    let (a, b) = tokio::try_join!(
        first.request::<GetAccountResponse>(request),
        second.request::<GetAccountResponse>(request)
    )?;
    assert_eq!(a, b);
    let auth = codex_login::load_auth_dot_json(
        home.path(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?
    .unwrap();
    let tokens = auth.tokens.unwrap();
    assert_eq!(
        (tokens.access_token, tokens.refresh_token),
        ("rotated-access".to_owned(), "rotated-refresh".to_owned())
    );
    Ok(())
}

#[tokio::test]
#[serial_test::serial]
async fn saved_connection_cancel_login_preserves_active_credentials_and_registry() -> Result<()> {
    let backend = MockServer::start().await;
    let home = tempfile::tempdir()?;
    write_config(home.path(), &backend)?;
    write_chatgpt_auth(
        home.path(),
        fixture("source"),
        AuthCredentialsStoreMode::File,
    )?;
    let original = std::fs::read(home.path().join("auth.json"))?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let original_account = account(&mut server).await?;
    let login = manage(
        &mut server,
        SavedConnectionParams::Add {
            provider: SavedConnectionProvider::Openai,
            name: "work".to_owned().try_into().unwrap(),
        },
    )
    .await?;
    let SavedConnectionResponse::LoginStarted { login_id, .. } = login else {
        anyhow::bail!("expected login challenge");
    };
    let canceled: CancelLoginAccountResponse = server
        .request(|request_id| ClientRequest::CancelLoginAccount {
            request_id,
            params: CancelLoginAccountParams {
                login_id: login_id.as_str().to_owned(),
            },
        })
        .await?;
    assert_eq!(canceled.status, CancelLoginAccountStatus::Canceled);
    let notification = tokio::time::timeout(
        Duration::from_secs(10),
        server.read_stream_until_notification_message("account/login/completed"),
    )
    .await??;
    let ServerNotification::AccountLoginCompleted(completed) = notification.try_into()? else {
        anyhow::bail!("expected completion");
    };
    assert_eq!(
        (completed.login_id, completed.success),
        (Some(login_id.as_str().to_owned()), false)
    );
    assert_eq!(account(&mut server).await?, original_account);
    assert_eq!(std::fs::read(home.path().join("auth.json"))?, original);
    let accounts = manage(
        &mut server,
        SavedConnectionParams::List {
            active_provider: "openai".to_owned().try_into().unwrap(),
        },
    )
    .await?;
    assert!(matches!(accounts, SavedConnectionResponse::Connections { data } if data.len() == 1));
    Ok(())
}

#[tokio::test]
async fn saved_connection_disconnect_restores_source_in_the_same_runtime() -> Result<()> {
    use super::connection_handling_websocket::connect_websocket;
    use super::connection_handling_websocket::read_error_for_id;
    use super::connection_handling_websocket::read_response_for_id;
    use super::connection_handling_websocket::send_initialize_request;
    use super::connection_handling_websocket::send_request;
    use super::connection_handling_websocket::spawn_websocket_server;
    let backend = MockServer::start().await;
    let home = tempfile::tempdir()?;
    write_config(home.path(), &backend)?;
    write_chatgpt_auth(
        home.path(),
        fixture("source"),
        AuthCredentialsStoreMode::File,
    )?;
    let store = ConnectionStore::new(
        home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        codex_login::AuthRouteConfig::from_http_client_factory(
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
        ),
    );
    let login = store.begin_login(ConnectionProvider::Openai, NonEmptyString::new("work")?)?;
    write_chatgpt_auth(
        login.home(),
        fixture("work"),
        AuthCredentialsStoreMode::File,
    )?;
    let saved = login.finish()?;
    let (mut process, address) = spawn_websocket_server(home.path()).await?;
    let mut owner = connect_websocket(address).await?;
    let mut observer = connect_websocket(address).await?;
    for (client, name) in [
        (&mut owner, "switch_owner"),
        (&mut observer, "switch_observer"),
    ] {
        send_initialize_request(client, 1, name).await?;
        read_response_for_id(client, 1).await?;
    }
    send_request(
        &mut observer,
        "account/read",
        2,
        Some(json!({"refreshToken": false})),
    )
    .await?;
    let source = read_response_for_id(&mut observer, 2).await?.result;
    let switch_id: ConnectionText = "disconnect-handoff".to_owned().try_into().unwrap();
    send_request(
        &mut owner,
        "savedConnection/manage",
        3,
        Some(serde_json::to_value(SavedConnectionParams::Select {
            id: saved.id().to_owned().try_into().unwrap(),
            switch_id: switch_id.clone(),
        })?),
    )
    .await?;
    read_response_for_id(&mut owner, 3).await?;
    send_request(
        &mut observer,
        "savedConnection/manage",
        4,
        Some(serde_json::to_value(SavedConnectionParams::Commit {
            switch_id,
        })?),
    )
    .await?;
    assert_eq!(
        read_error_for_id(&mut observer, 4).await?.error.code,
        -32600
    );
    owner.close(None).await?;
    drop(owner);
    tokio::time::timeout(Duration::from_secs(10), async {
        for request_id in 10.. {
            send_request(
                &mut observer,
                "account/read",
                request_id,
                Some(json!({"refreshToken": false})),
            )
            .await?;
            if read_response_for_id(&mut observer, request_id)
                .await?
                .result
                == source
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        unreachable!()
    })
    .await??;
    assert!(process.try_wait()?.is_none());
    assert!(!home.path().join("connections/selected.json").exists());
    Ok(())
}
