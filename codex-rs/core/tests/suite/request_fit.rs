use anyhow::Result;
use codex_login::CodexAuth;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use wiremock::MockServer;

use super::model_switching::read_only_user_turn;
use super::model_switching::test_model_info;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn normalized_history_that_fits_does_not_trigger_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let multimodal_model_slug = "request-fit-multimodal";
    let text_model_slug = "request-fit-text";
    let models = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![
                test_model_info(
                    multimodal_model_slug,
                    "Request Fit Multimodal",
                    "supports images",
                    vec![InputModality::Text, InputModality::Image],
                ),
                test_model_info(
                    text_model_slug,
                    "Request Fit Text",
                    "supports text",
                    vec![InputModality::Text],
                ),
            ],
        },
    )
    .await;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("message-1", "image retained"),
                responses::ev_completed_with_tokens("resp-1", /*total_tokens*/ 1_500),
            ]),
            responses::sse(vec![responses::ev_completed("resp-2")]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.model = Some(multimodal_model_slug.to_string());
            config.model_auto_compact_token_limit = Some(1_000);
        });
    let test = builder.build_with_auto_env(&server).await?;
    let _available_models = test
        .thread_manager
        .get_models_manager()
        .list_models(
            codex_models_manager::manager::RefreshStrategy::OnlineIfUncached,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;
    assert_eq!(models.requests().len(), 1);
    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![
                UserInput::Image {
                    image_url: image_url.to_string(),
                    detail: None,
                },
                UserInput::Text {
                    text: "retain this image in history".to_string(),
                    text_elements: Vec::new(),
                },
            ],
            multimodal_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "continue with text only".to_string(),
                text_elements: Vec::new(),
            }],
            text_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let second = requests.last().expect("second sampling request");
    let second_body = second.body_json();
    assert!(
        second.inputs_of_type("compaction_trigger").is_empty(),
        "unexpected compaction request: {second_body:#}"
    );
    assert_eq!(second_body["model"], text_model_slug);
    assert!(second.message_input_image_urls("user").is_empty());
    assert!(
        second
            .message_input_texts("user")
            .iter()
            .any(|text| text == "continue with text only")
    );

    Ok(())
}
