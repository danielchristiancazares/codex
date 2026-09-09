use super::*;
use crate::AppServerTarget;
use crate::RemoteAppServerEndpoint;
use crate::status::RuntimeProviderStatus;

#[tokio::test]
async fn status_renders_sanitized_local_endpoint_and_authoritative_remote_identity() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model_provider_id = "proxy".to_string();
    config.model_provider = ModelProviderInfo {
        name: "Local Proxy".to_string(),
        base_url: Some("https://user:password@example.test/v1/?api_key=secret#private".to_string()),
        ..ModelProviderInfo::default()
    };
    let now = Local
        .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
        .single()
        .expect("timestamp");
    let usage = TokenUsage::default();
    let mut rows = Vec::new();
    for (label, target) in [
        ("local", AppServerTarget::Embedded),
        (
            "remote",
            AppServerTarget::Remote {
                endpoint: RemoteAppServerEndpoint::UnixSocket {
                    socket_path: temp_home.path().join("remote.sock").abs(),
                },
            },
        ),
    ] {
        let provider_status = RuntimeProviderStatus::resolve(&config, &target).await;
        let (card, _) = new_status_output_with_rate_limits_handle(
            &config,
            /*requires_openai_auth*/ false,
            Some("proxy"),
            &provider_status,
            /*remote_connection*/ None,
            /*account_display*/ None,
            /*token_info*/ None,
            &usage,
            /*session_id*/ &None,
            /*thread_name*/ None,
            /*forked_from*/ None,
            /*rate_limits*/ &[],
            /*_plan_type*/ None,
            now,
            "model",
            /*collaboration_mode*/ None,
            /*reasoning_effort_override*/ None,
            "<none>".to_string(),
            /*refreshing_rate_limits*/ false,
        );
        let rendered = render_lines(&card.display_lines(/*width*/ 120));
        let provider_row = rendered
            .iter()
            .find(|line| line.contains("Model provider:"))
            .expect("provider row");
        rows.push(format!(
            "{label}: {}",
            provider_row
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    assert_snapshot!(rows.join("\n"), @r"
    local: Model provider: Local Proxy - https://example.test/v1
    remote: Model provider: proxy
    ");
}
