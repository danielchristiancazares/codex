use super::without_notification_media;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use pretty_assertions::assert_eq;

fn notifications(output: FunctionCallOutputBody) -> [ServerNotification; 2] {
    let item = ThreadItem::FunctionCallOutput {
        id: "item".to_string(),
        name: "tool".to_string(),
        namespace: Some("namespace".to_string()),
        output,
    };
    [
        ServerNotification::ItemStarted(ItemStartedNotification {
            item: item.clone(),
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
            started_at_ms: 123,
        }),
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            item,
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
            completed_at_ms: 456,
        }),
    ]
}

#[test]
fn function_output_notifications_filter_media_and_preserve_other_content_and_metadata() {
    let text = FunctionCallOutputContentItem::InputText {
        text: "retained text".to_string(),
    };
    let encrypted = FunctionCallOutputContentItem::EncryptedContent {
        encrypted_content: "retained ciphertext".to_string(),
    };
    let image = FunctionCallOutputContentItem::InputImage {
        image_url: "data:image/png;base64,image".to_string(),
        detail: None,
    };
    let audio = FunctionCallOutputContentItem::InputAudio {
        audio_url: "data:audio/wav;base64,audio".to_string(),
    };

    for (output, expected) in [
        (
            FunctionCallOutputBody::ContentItems(vec![
                text.clone(),
                image.clone(),
                encrypted.clone(),
                audio.clone(),
            ]),
            FunctionCallOutputBody::ContentItems(vec![text, encrypted]),
        ),
        (
            FunctionCallOutputBody::ContentItems(vec![image, audio]),
            FunctionCallOutputBody::ContentItems(Vec::new()),
        ),
        (
            FunctionCallOutputBody::Text("plain output".to_string()),
            FunctionCallOutputBody::Text("plain output".to_string()),
        ),
    ] {
        for (notification, expected) in notifications(output)
            .into_iter()
            .zip(notifications(expected))
        {
            assert_eq!(
                serde_json::to_value(without_notification_media(notification))
                    .expect("serialize filtered notification"),
                serde_json::to_value(expected).expect("serialize expected notification"),
            );
        }
    }
}
