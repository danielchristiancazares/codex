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
use codex_utils_output_truncation::truncate_text;
use futures::StreamExt;

enum RejectionPolicy {
    ReduceOnce,
    StopAfterReduction,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("local compaction cannot reduce history further without dropping the newest turn")]
pub(super) struct CompactionReductionExhausted;

#[derive(Debug, thiserror::Error)]
#[error("local compaction replacement cannot fit its required context in the model window")]
pub(super) struct ReplacementExceedsWindow;

pub(super) struct LocalCompactionPlan {
    history: ContextManager,
    compaction_input_items: usize,
    rejection_policy: RejectionPolicy,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct LocalCompactionReduction {
    pub(super) removed_groups: usize,
    pub(super) removed_items: usize,
    pub(super) estimated_tokens_before: i64,
    pub(super) estimated_tokens_after: i64,
    pub(super) target_tokens: i64,
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
            rejection_policy: RejectionPolicy::ReduceOnce,
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
        match std::mem::replace(
            &mut self.rejection_policy,
            RejectionPolicy::StopAfterReduction,
        ) {
            RejectionPolicy::ReduceOnce => {}
            RejectionPolicy::StopAfterReduction => return Err(CompactionReductionExhausted),
        }

        let items = self.history.annotated_items();
        if items.len() <= self.compaction_input_items {
            return Err(CompactionReductionExhausted);
        }
        // The summarization instruction belongs to the newest real turn group.
        let retained_source_len = items.len().saturating_sub(self.compaction_input_items);
        let group_starts = item_group_starts(&items[..retained_source_len]);
        if group_starts.len() <= 1 {
            return Err(CompactionReductionExhausted);
        }

        let base_tokens =
            i64::try_from(approx_token_count(&base_instructions.text)).unwrap_or(i64::MAX);
        let estimated_tokens_before = base_tokens.saturating_add(estimated_items_tokens(items));
        let target_tokens = budget.reduced_target(estimated_tokens_before);

        let protected_group_index = group_starts.len() - 1;
        let mut estimated_tokens_after = estimated_tokens_before;
        let mut retained_start = 0;
        let mut removed_groups = 0;
        for group_index in 0..protected_group_index {
            if estimated_tokens_after <= target_tokens {
                break;
            }
            let group_start = group_starts[group_index];
            let group_end = group_starts[group_index + 1];
            estimated_tokens_after = estimated_tokens_after
                .saturating_sub(estimated_items_tokens(&items[group_start..group_end]));
            retained_start = group_end;
            removed_groups += 1;
        }
        if removed_groups == 0 {
            return Err(CompactionReductionExhausted);
        }

        let removed_items = retained_start;
        let retained = items[retained_start..].to_vec();
        self.history.replace_annotated(retained);
        Ok(LocalCompactionReduction {
            removed_groups,
            removed_items,
            estimated_tokens_before,
            estimated_tokens_after,
            target_tokens,
        })
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
        // Every candidate includes the preserved instruction/publication envelopes.
        // Seed each bounded search with a fitting concrete candidate.
        let summary_token_limit = approx_token_count(summary_text).min(10_000);
        let mut bounded_summary = canonical_compaction_summary_text("");
        budget
            .check(estimated_request_tokens(
                base_instructions,
                &replacement_candidate(&initial_context, &[], &bounded_summary, 0),
            ))
            .map_err(|_| ReplacementExceedsWindow)?;
        let mut lower = 0usize;
        let mut upper = summary_token_limit;
        while lower <= upper {
            let allowance = lower + (upper - lower) / 2;
            let candidate = canonical_compaction_summary_text(&truncate_text(
                summary_text,
                TruncationPolicy::Tokens(allowance),
            ));
            let items = replacement_candidate(&initial_context, &[], &candidate, 0);
            if approx_token_count(&candidate) <= 10_000
                && budget
                    .check(estimated_request_tokens(base_instructions, &items))
                    .is_ok()
            {
                bounded_summary = candidate;
                lower = allowance.saturating_add(1);
            } else if allowance == 0 {
                break;
            } else {
                upper = allowance - 1;
            }
        }
        let mut fitted_items = replacement_candidate(&initial_context, &[], &bounded_summary, 0);
        let mut lower = 0usize;
        let mut upper = COMPACT_USER_MESSAGE_MAX_TOKENS;
        while lower <= upper {
            let allowance = lower + (upper - lower) / 2;
            let items = replacement_candidate(
                &initial_context,
                &user_messages,
                &bounded_summary,
                allowance,
            );
            if budget
                .check(estimated_request_tokens(base_instructions, &items))
                .is_ok()
            {
                fitted_items = items;
                lower = allowance.saturating_add(1);
            } else if allowance == 0 {
                break;
            } else {
                upper = allowance - 1;
            }
        }
        Ok(LocalCompactionReplacement {
            items: fitted_items,
            summary_text: NonEmptyString::summary(&bounded_summary),
        })
    }
}

fn replacement_candidate(
    initial_context: &[ResponseItemEnvelope],
    user_messages: &[super::CompactedUserMessage],
    summary_text: &str,
    user_message_token_limit: usize,
) -> Vec<ResponseItemEnvelope> {
    let history = build_compacted_history_with_limit(
        Vec::new(),
        user_messages,
        summary_text,
        user_message_token_limit,
    );
    insert_initial_context_before_last_real_user_or_summary(history, initial_context.to_vec())
}

fn item_group_starts(items: &[ResponseItemEnvelope]) -> Vec<usize> {
    let mut starts = Vec::new();
    if !items.is_empty() {
        starts.push(0);
    }
    for (index, envelope) in items.iter().enumerate().skip(1) {
        if is_user_turn_boundary(&envelope.item) {
            starts.push(index);
        }
    }
    starts
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
