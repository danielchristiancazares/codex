use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

fn factory() -> HttpClientFactory {
    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
}

#[tokio::test]
async fn device_code_request_uses_the_native_copilot_oauth_contract() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/device/code"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(header("accept", "application/json"))
        .and(header("user-agent", COPILOT_USER_AGENT))
        .and(body_string_contains(&format!(
            "client_id={GITHUB_CLIENT_ID}"
        )))
        .and(body_string_contains("scope=read%3Auser"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "device-secret",
            "user_code": "ABCD-EFGH",
            "verification_uri": format!("{}/login/device", server.uri()),
            "expires_in": 900,
            "interval": 5
        })))
        .expect(1)
        .mount(&server)
        .await;
    let endpoints = OAuthEndpoints::from_base_url(&server.uri());

    let device_code = request_device_code(&factory(), &endpoints)
        .await
        .expect("request device code");

    assert_eq!(device_code.user_code, "ABCD-EFGH");
    assert_eq!(
        device_code.verification_uri,
        format!("{}/login/device", server.uri())
    );
    assert_eq!(device_code.poll_interval, Duration::from_secs(5));
}

#[test]
fn device_code_rejects_a_verification_origin_change() {
    let endpoints = OAuthEndpoints::from_base_url("https://github.com");
    let result = validate_device_code(
        DeviceCodeResponse {
            device_code: "device-secret".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "https://example.com/login/device".to_string(),
            expires_in: 900,
            interval: 5,
        },
        &endpoints,
    );

    assert_eq!(
        result
            .expect_err("foreign verification origin must fail")
            .to_string(),
        "GitHub device-code response contained an unexpected verification URL"
    );
}
