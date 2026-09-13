use super::COMPACT_USER_MESSAGE_MAX_TOKENS;
use super::CompactedMessageIdentity;
use super::build_compacted_history_with_limit;
use super::canonical_compaction_summary_text;
use super::collect_annotated_user_messages;
use super::insert_initial_context_before_last_real_user_or_summary;
use crate::Prompt;
use crate::client::ModelClientSession;
use crate::client_common::ResponseEvent;
use crate::context_manager::ContextManager;
use crate::context_manager::RequestBudget;
use crate::context_manager::estimate_item_token_count;
use crate::context_manager::is_user_turn_boundary;
use crate::context_manager::strip_tool_search_schemas;
use crate::responses_metadata::CodexResponsesMetadata;
use crate::session::RequestEffortUsage;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_context_policy::CompactionItem;
use codex_context_policy::CompactionReductionExhausted;
use codex_context_policy::LocalCompactionReduction;
use codex_context_policy::ReplacementExceedsWindow;
use codex_context_policy::fit_compaction_replacement;
use codex_context_policy::plan_compaction_reduction;
use codex_history::ResponseItemEnvelope;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use codex_rollout_trace::InferenceTraceContext;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use futures::StreamExt;

pub(super) struct LocalCompactionPlan {
    history: ContextManager,
    compaction_input_items: usize,
}

pub(super) struct NonEmptyString(String);

impl NonEmptyString {
    fn summary(value: &str) -> Self {
        Self(canonical_compaction_summary_text(value))
    }

    fn response_id(value: String) -> CodexResult<Self> {
        if value.trim().is_empty() {
            return Err(CodexErr::new(CodexErrorDetails::ResponseProtocol(
                codex_protocol::ResponseProtocolFailure::for_event(
                    "response.completed",
                    "compaction response id must be nonempty",
                    "",
                ),
            )));
        }
        Ok(Self(value))
    }

    pub(super) fn into_string(self) -> String {
        self.0
    }
}

pub(super) struct LocalCompactionOutput {
    pub(super) items: Vec<ResponseItem>,
    pub(super) response_id: NonEmptyString,
}

pub(super) struct LocalCompactionReplacement {
    pub(super) items: Vec<ResponseItemEnvelope>,
    pub(super) summary_text: NonEmptyString,
}

impl LocalCompactionPlan {
    pub(super) fn new(
        mut history: ContextManager,
        compaction_input: ResponseItem,
        truncation_policy: TruncationPolicy,
    ) -> Self {
        let item_count_before = history.raw_items().len();
        history.record_items(std::slice::from_ref(&compaction_input), truncation_policy);
        let compaction_input_items = history.raw_items().len().saturating_sub(item_count_before);
        Self {
            history,
            compaction_input_items,
        }
    }

    pub(super) fn prompt_input(&self, input_modalities: &[InputModality]) -> Vec<ResponseItem> {
        let mut input = self.history.clone().for_prompt(input_modalities);
        strip_tool_search_schemas(&mut input);
        input
    }

    pub(super) fn reduce_after_context_error(
        &mut self,
        base_instructions: &BaseInstructions,
        budget: RequestBudget,
    ) -> Result<LocalCompactionReduction, CompactionReductionExhausted> {
        let items = self.history.annotated_items();
        let measured = items
            .iter()
            .map(|envelope| CompactionItem {
                item: &envelope.item,
                estimated_tokens: estimate_item_token_count(&envelope.item),
                starts_turn: is_user_turn_boundary(&envelope.item),
            })
            .collect::<Vec<_>>();
        let base_tokens =
            i64::try_from(approx_token_count(&base_instructions.text)).unwrap_or(i64::MAX);
        let decision = plan_compaction_reduction(
            &measured,
            self.compaction_input_items,
            base_tokens,
            budget,
        )?;
        let retained = decision
            .retained_indices
            .into_iter()
            .map(|index| items[index].clone())
            .collect();
        self.history.replace_annotated(retained);
        Ok(decision.reduction)
    }

    pub(super) fn build_replacement(
        &self,
        initial_context: Vec<ResponseItemEnvelope>,
        summary_text: &str,
        base_instructions: &BaseInstructions,
        budget: RequestBudget,
        identity: CompactedMessageIdentity,
    ) -> Result<LocalCompactionReplacement, ReplacementExceedsWindow> {
        let retained_source_len = self
            .history
            .annotated_items()
            .len()
            .saturating_sub(self.compaction_input_items);
        let user_messages = collect_annotated_user_messages(
            &self.history.annotated_items()[..retained_source_len],
            identity,
        );
        let replacement = fit_compaction_replacement(
            summary_text,
            COMPACT_USER_MESSAGE_MAX_TOKENS,
            budget,
            canonical_compaction_summary_text,
            |summary, user_message_token_limit| {
                // Every candidate includes preserved instruction/publication envelopes.
                let history = build_compacted_history_with_limit(
                    Vec::new(),
                    &user_messages,
                    summary,
                    user_message_token_limit,
                );
                let items = insert_initial_context_before_last_real_user_or_summary(
                    history,
                    initial_context.clone(),
                );
                let tokens = estimated_request_tokens(base_instructions, &items);
                (items, tokens)
            },
        )?;
        Ok(LocalCompactionReplacement {
            items: replacement.items,
            summary_text: NonEmptyString::summary(&replacement.summary_text),
        })
    }
}

fn estimated_items_tokens(items: &[ResponseItemEnvelope]) -> i64 {
    items
        .iter()
        .map(|envelope| estimate_item_token_count(&envelope.item))
        .fold(0i64, i64::saturating_add)
}

fn estimated_request_tokens(
    base_instructions: &BaseInstructions,
    items: &[ResponseItemEnvelope],
) -> i64 {
    i64::try_from(approx_token_count(&base_instructions.text))
        .unwrap_or(i64::MAX)
        .saturating_add(estimated_items_tokens(items))
}

pub(super) async fn drain_to_completed(
    sess: &Session,
    turn_context: &TurnContext,
    client_session: &mut ModelClientSession,
    responses_metadata: &CodexResponsesMetadata,
    prompt: &Prompt,
) -> CodexResult<LocalCompactionOutput> {
    let mut stream = client_session
        .stream(
            prompt,
            turn_context.model_info(),
            &turn_context.session_telemetry,
            sess.reasoning_effort_for_request(
                &turn_context.initial_settings,
                RequestEffortUsage::Compaction,
            )
            .await,
            turn_context.reasoning_summary(),
            turn_context.config.service_tier.clone(),
            responses_metadata,
            // Rollout tracing currently models remote compaction only; local compaction streams
            // are left untraced until the reducer has a first-class local compaction lifecycle.
            &InferenceTraceContext::disabled(),
        )
        .await?;
    let mut items = Vec::new();
    loop {
        let Some(event) = stream.next().await else {
            return Err(CodexErr::Stream(
                "stream closed before response.completed".into(),
            ));
        };
        match event {
            Ok(ResponseEvent::OutputItemDone(item)) => items.push(item),
            Ok(ResponseEvent::ServerReasoningIncluded(included)) => {
                sess.set_server_reasoning_included(included).await;
            }
            Ok(ResponseEvent::RateLimits(snapshot)) => {
                sess.update_rate_limits(turn_context, snapshot).await;
            }
            Ok(ResponseEvent::Completed {
                response_id,
                token_usage,
                usage_metadata,
                ..
            }) => {
                sess.record_observed_response_completed(
                    turn_context,
                    &response_id,
                    token_usage.as_ref(),
                    usage_metadata.as_ref(),
                )
                .await;
                sess.update_token_usage_info(turn_context, token_usage.as_ref())
                    .await?;
                return Ok(LocalCompactionOutput {
                    items,
                    response_id: NonEmptyString::response_id(response_id)?,
                });
            }
            Ok(_) => continue,
            Err(error) => {
                if let codex_protocol::error::CodexErrorDetails::IncompleteResponse(failure) =
                    error.details()
                {
                    failure
                        .record_usage(|usage| {
                            sess.update_token_usage_info(turn_context, Some(usage))
                        })
                        .await;
                }
                return Err(error);
            }
        }
    }
}

#[cfg(test)]
#[path = "local_plan_tests.rs"]
mod tests;
