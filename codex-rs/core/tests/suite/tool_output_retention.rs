use anyhow::Context;
use codex_history::RolloutItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_rollout::RolloutRecorder;
use codex_utils_image::data_url_from_bytes;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use image::DynamicImage;
use image::ImageBuffer;
use image::ImageFormat;
use image::Rgba;
use pretty_assertions::assert_eq;
use std::io::Cursor;

const TEXT_CALL_ID: &str = "historical-large-text";
const IMAGE_CALL_ID: &str = "historical-images";

fn function_call(name: &str, call_id: &str) -> ResponseItem {
    ResponseItem::FunctionCall {
        id: None,
        name: name.to_string(),
        namespace: None,
        arguments: "{}".to_string(),
        encrypted_function_args: None,
        call_id: call_id.to_string(),
        internal_chat_message_metadata_passthrough: None,
    }
}

fn function_output(call_id: &str, output: FunctionCallOutputPayload) -> ResponseItem {
    ResponseItem::FunctionCallOutput {
        id: None,
        call_id: Some(call_id.to_string()),
        name: None,
        namespace: None,
        output,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn large_image_data_url() -> anyhow::Result<String> {
    let image = ImageBuffer::from_pixel(
        /*width*/ 2304,
        /*height*/ 864,
        Rgba([12u8, 34, 56, 255]),
    );
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut encoded, ImageFormat::Png)?;
    Ok(data_url_from_bytes("image/png", &encoded.into_inner()))
}

fn resize_notices(input: &responses::ResponsesRequest) -> Vec<String> {
    input
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<image_resize_notice>"))
        .collect()
}

fn persisted_function_output<'a>(
    history: &'a [RolloutItem],
    call_id: &str,
) -> Option<&'a FunctionCallOutputPayload> {
    history.iter().find_map(|item| {
        let RolloutItem::ResponseItem(envelope) = item else {
            return None;
        };
        match &envelope.item {
            ResponseItem::FunctionCallOutput {
                call_id: Some(persisted_call_id),
                output,
                ..
            } if persisted_call_id == call_id => Some(output),
            _ => None,
        }
    })
}

fn persisted_resize_notices(history: &[RolloutItem]) -> Vec<String> {
    history
        .iter()
        .filter_map(|item| {
            let RolloutItem::ResponseItem(envelope) = item else {
                return None;
            };
            let ResponseItem::Message { role, content, .. } = &envelope.item else {
                return None;
            };
            if role != "developer" {
                return None;
            }
            content.iter().find_map(|item| match item {
                ContentItem::InputText { text } if text.starts_with("<image_resize_notice>") => {
                    Some(text.clone())
                }
                _ => None,
            })
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_output_retention_survives_resume_without_stale_notices() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut initial_builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .features
            .enable(codex_features::Feature::ImageResizeNotice)
            .expect("image resize notices should be configurable");
    });
    let initial = initial_builder.build_with_auto_env(&server).await?;

    let raw_text = "dense historical tool output !@#$%^&* ".repeat(10_000);
    let image_url = large_image_data_url()?;
    initial
        .codex
        .inject_response_items(vec![
            function_call("large_text", TEXT_CALL_ID),
            function_output(
                TEXT_CALL_ID,
                FunctionCallOutputPayload::from_text(raw_text.clone()),
            ),
            function_call("many_images", IMAGE_CALL_ID),
            function_output(
                IMAGE_CALL_ID,
                FunctionCallOutputPayload::from_content_items(
                    (0..5)
                        .map(|_| FunctionCallOutputContentItem::InputImage {
                            image_url: image_url.clone(),
                            detail: Some(ImageDetail::High),
                        })
                        .collect(),
                ),
            ),
        ])
        .await?;

    let live_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-live"),
            responses::ev_assistant_message("msg-live", "recorded"),
            responses::ev_completed("resp-live"),
        ]),
    )
    .await;
    initial.submit_turn("record retained tool outputs").await?;
    let live_request = live_mock.single_request();
    let live_text_output = live_request.function_call_output(TEXT_CALL_ID);
    let live_text = live_text_output["output"]
        .as_str()
        .context("live text output")?
        .to_string();
    let live_image_output = live_request.function_call_output(IMAGE_CALL_ID);
    let live_images = live_image_output["output"]
        .as_array()
        .context("live image output")?;
    assert!(live_text.len() < raw_text.len());
    assert_eq!(
        live_images
            .iter()
            .filter(|item| item["type"] == "input_image")
            .count(),
        4
    );
    let live_notices = resize_notices(&live_request);
    let [live_notice] = live_notices.as_slice() else {
        anyhow::bail!("expected one live resize notice, got {live_notices:?}");
    };
    assert_eq!(live_notice.matches("Image ").count(), 4);
    assert!(live_notice.contains("Image 4 of 4"));
    assert!(!live_notice.contains(" of 5 "));

    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .context("initial rollout path")?;
    let home = initial.home.clone();
    initial.codex.shutdown_and_wait().await?;
    let persisted = RolloutRecorder::get_rollout_history(&rollout_path).await?;
    let persisted_items = persisted.get_rollout_items();
    let persisted_text = persisted_function_output(persisted_items, TEXT_CALL_ID)
        .context("persisted text output")?;
    let persisted_images = persisted_function_output(persisted_items, IMAGE_CALL_ID)
        .context("persisted image output")?;
    assert_eq!(
        serde_json::to_value(persisted_text)?,
        serde_json::Value::String(live_text.clone())
    );
    assert_eq!(
        serde_json::to_value(persisted_images)?,
        live_image_output["output"]
    );
    assert_eq!(persisted_resize_notices(persisted_items), live_notices);

    let mut resumed_builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config.tool_output_token_limit = Some(100_000);
        config
            .features
            .enable(codex_features::Feature::ImageResizeNotice)
            .expect("image resize notices should be configurable");
    });
    let resumed = resumed_builder.resume(&server, home, rollout_path).await?;
    let resumed_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-resumed"),
            responses::ev_assistant_message("msg-resumed", "done"),
            responses::ev_completed("resp-resumed"),
        ]),
    )
    .await;
    resumed.submit_turn("inspect retained tool outputs").await?;
    let resumed_request = resumed_mock.single_request();

    assert_eq!(
        resumed_request.function_call_output(TEXT_CALL_ID)["output"],
        serde_json::Value::String(live_text)
    );
    assert_eq!(
        resumed_request.function_call_output(IMAGE_CALL_ID)["output"],
        live_image_output["output"]
    );
    assert_eq!(resize_notices(&resumed_request), live_notices);
    Ok(())
}
