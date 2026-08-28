use super::DEFAULT_READ_TIMEOUT;
use super::TINY_PNG_DATA_URL;
use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;

#[derive(Clone, Copy)]
enum NotificationMedia {
    Included,
    Omitted,
}

#[test_case(NotificationMedia::Included; "default preserves notification media")]
#[test_case(NotificationMedia::Omitted; "opt in omits notification media")]
#[tokio::test]
async fn turn_start_notification_media_preserves_model_input(
    media: NotificationMedia,
) -> Result<()> {
    let server = responses::start_websocket_server(vec![vec![
        vec![
            responses::ev_response_created("warmup"),
            responses::ev_completed("warmup"),
        ],
        vec![
            responses::ev_response_created("response"),
            responses::ev_assistant_message("message", "Done"),
            responses::ev_completed("response"),
        ],
    ]])
    .await;
    let codex_home = TempDir::new()?;
    let base_url = server.uri().replacen("ws://", "http://", 1);
    let config =
        MockResponsesConfig::new(&base_url).with_provider_config("supports_websockets = true");
    let config = match media {
        NotificationMedia::Included => config,
        NotificationMedia::Omitted => {
            config.enable_feature(Feature::OmitAppServerNotificationMedia)
        }
    };
    config.write(codex_home.path())?;

    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = app
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            experimental_raw_events: true,
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id,
                input: vec![
                    UserInput::Text {
                        text: "Describe this image".to_string(),
                        text_elements: Vec::new(),
                    },
                    UserInput::Image {
                        url: TINY_PNG_DATA_URL.to_string(),
                        detail: None,
                    },
                ],
                ..Default::default()
            },
        })
        .await?;

    let mut user_message_notifications = Vec::new();
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification = app
                .read_stream_until_matching_notification(
                    "item notification or turn completion",
                    |notification| {
                        matches!(
                            notification.method.as_str(),
                            "item/started"
                                | "item/completed"
                                | "rawResponseItem/completed"
                                | "turn/completed"
                        )
                    },
                )
                .await?;
            if notification.method == "turn/completed" {
                return Ok::<(), anyhow::Error>(());
            }
            let params = notification.params.context("item notification params")?;
            let item = &params["item"];
            if item["type"] != "userMessage"
                && !(item["type"] == "message" && item["role"] == "user")
            {
                continue;
            }
            let content = item["content"].as_array().context("user message content")?;
            if !content
                .iter()
                .any(|item| item["text"] == "Describe this image")
            {
                continue;
            }
            match media {
                NotificationMedia::Included => {
                    assert_eq!(content.len(), 2);
                    assert!(content.iter().any(|item| matches!(
                        item["type"].as_str(),
                        Some("image" | "input_image")
                    )));
                }
                NotificationMedia::Omitted => {
                    assert_eq!(content.len(), 1);
                    assert!(matches!(
                        content[0]["type"].as_str(),
                        Some("text" | "input_text")
                    ));
                }
            }
            user_message_notifications.push(notification.method);
        }
    })
    .await??;
    user_message_notifications.sort();
    assert_eq!(
        user_message_notifications,
        vec![
            "item/completed",
            "item/started",
            "rawResponseItem/completed"
        ]
    );

    let requests = server.single_connection();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body_json()["generate"], false);
    let body = requests[1].body_json();
    let model_input_images: Vec<_> = body["input"]
        .as_array()
        .context("model input")?
        .iter()
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .filter(|content| content["type"] == "input_image")
        .collect();
    assert_eq!(model_input_images.len(), 1);
    assert_eq!(model_input_images[0]["image_url"], TINY_PNG_DATA_URL);
    assert!(app.shutdown_gracefully().await?.success());
    server.shutdown().await;
    Ok(())
}
