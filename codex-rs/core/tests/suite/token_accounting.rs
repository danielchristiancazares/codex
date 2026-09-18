use anyhow::Result;
use codex_features::Feature;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn context_budget_preserves_reported_usage_after_hidden_citations_are_filtered() -> Result<()>
{
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(10_000);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("enable token budget");
        })
        .build_with_auto_env(&server)
        .await?;
    let first = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message(
                "cited-answer",
                &format!(
                    "Answer.<oai-mem-citation>{}</oai-mem-citation>",
                    "hidden reference".repeat(/*n*/ 400)
                ),
            ),
            responses::ev_completed_with_tokens("first", /*total_tokens*/ 2_500),
        ]),
    )
    .await;
    test.submit_turn("Give a cited answer.").await?;
    first.single_request();

    let call_id = "remaining";
    let follow_up = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(call_id, "get_context_remaining", "{}"),
                responses::ev_completed_with_tokens("second", /*total_tokens*/ 2_500),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("final-answer", "Done."),
                responses::ev_completed("third"),
            ]),
        ],
    )
    .await;
    test.submit_turn("Check the remaining context.").await?;

    let requests = follow_up.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0]
            .inputs_of_type("message")
            .into_iter()
            .filter(|message| message["role"] == "assistant")
            .map(|message| message["content"].clone())
            .collect::<Vec<_>>(),
        vec![json!([{"type": "output_text", "text": "Answer."}])]
    );
    assert_eq!(
        requests[1].function_call_output_content_and_success(call_id),
        Some((
            Some("You have 6500 tokens left in this context window.".to_string()),
            None,
        ))
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
