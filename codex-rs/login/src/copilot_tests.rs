use std::sync::Arc;

use codex_http_client::OutboundProxyPolicy;
use codex_keyring_store::tests::MockKeyringStore;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

const TEST_MACHINE_ID: &str = "4f8c2f5df054b1e465c8f9d9af3b391a4718b02ad7c3d0f8e83d4f6978de1451";

#[test]
fn credential_debug_output_redacts_the_oauth_token() {
    let credential = GitHubCopilotCredential {
        token: "github-secret".to_string(),
        machine_id: TEST_MACHINE_ID.to_string(),
    };

    assert_eq!(
        format!("{credential:?}"),
        format!(
            "GitHubCopilotCredential {{ token: \"[REDACTED]\", machine_id: \
             \"{TEST_MACHINE_ID}\" }}"
        )
    );
}

#[tokio::test]
async fn login_authorizes_validates_and_persists_one_bound_credential() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/device/code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "device-secret",
            "user_code": "ABCD-EFGH",
            "verification_uri": format!("{}/login/device", server.uri()),
            "expires_in": 30,
            "interval": 1
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "github-secret",
            "token_type": "bearer"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", "Bearer github-secret"))
        .and(header("x-github-api-version", GITHUB_REST_API_VERSION))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": "octocat"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let auth = GitHubCopilotAuth::new_with_parts(
        codex_home.path(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        keyring,
        OAuthEndpoints::from_base_url(&server.uri()),
        server.uri(),
    );

    let login = auth.login(/*force*/ true).await.expect("complete login");
    let credential = auth
        .credential()
        .expect("load credential")
        .expect("stored credential");

    assert_eq!(login, "octocat");
    assert_eq!(credential.token(), "github-secret");
    assert_eq!(credential.machine_id().len(), 64);
}

#[tokio::test]
async fn non_forced_login_reuses_and_validates_the_stored_credential() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", "Bearer stored-secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "login": "octocat"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let auth = GitHubCopilotAuth::new_with_parts(
        codex_home.path(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        keyring,
        OAuthEndpoints::from_base_url(&server.uri()),
        server.uri(),
    );
    auth.credential_store
        .save(&GitHubCopilotCredential {
            token: "stored-secret".to_string(),
            machine_id: TEST_MACHINE_ID.to_string(),
        })
        .expect("seed stored credential");

    let login = auth.login(/*force*/ false).await.expect("reuse login");

    assert_eq!(login, "octocat");
}

#[test]
fn loaded_credential_remains_in_memory_when_the_store_changes() {
    let codex_home = tempfile::tempdir().expect("temporary Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let auth = GitHubCopilotAuth::new_with_parts(
        codex_home.path(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        keyring,
        OAuthEndpoints::github(),
        GITHUB_API_BASE_URL.to_string(),
    );
    let expected = GitHubCopilotCredential {
        token: "stored-secret".to_string(),
        machine_id: TEST_MACHINE_ID.to_string(),
    };
    auth.credential_store
        .save(&expected)
        .expect("seed stored credential");

    assert_eq!(auth.credential(), Ok(Some(expected.clone())));
    assert_eq!(auth.credential_store.delete(), Ok(true));
    assert_eq!(auth.credential(), Ok(Some(expected)));
}
