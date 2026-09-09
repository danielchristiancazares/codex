use super::NonEmptyString;
use super::Presentation;
use super::RuntimeProviderStatus;
use super::sanitize_endpoint;
use pretty_assertions::assert_eq;

#[test]
fn endpoint_removes_credentials_and_request_secrets() {
    let endpoint =
        sanitize_endpoint(" https://user:password@example.test/v1/?api_key=secret#private ")
            .expect("sanitized endpoint");
    assert_eq!(endpoint.0, "https://example.test/v1");
}

#[test]
fn opaque_or_malformed_endpoints_are_rejected() {
    for raw in [
        "",
        "invalid endpoint",
        "data:text/plain,private",
        "mailto:user@example.test",
    ] {
        assert!(sanitize_endpoint(raw).is_err(), "accepted {raw:?}");
    }
}

#[test]
fn provider_transition_uses_the_current_server_identity() {
    let presentation = RuntimeProviderStatus(Presentation::ShowLocalProvider {
        provider_id: NonEmptyString::new("local-proxy").expect("provider id"),
        label: NonEmptyString::new("Local Proxy - https://example.test/v1").expect("label"),
    });
    assert_eq!(
        [
            presentation.render("local-proxy"),
            presentation.render("remote-provider")
        ],
        ["Local Proxy - https://example.test/v1", "remote-provider"]
    );
}
