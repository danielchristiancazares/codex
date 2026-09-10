//! Delivery of completed hook results, including rollback-owned queue filtering.
use super::*;

pub(crate) enum AsyncHookDelivery<'a> {
    BeforeUserPrompt,
    ActiveTurn,
    AfterRollback(&'a std::collections::HashSet<String>),
}

/// Processes finished async hook results at a safe turn boundary.
///
/// Before the user prompt, records additional context directly into conversation
/// history so results from a previous turn appear before the new prompt. After
/// sampling, injects context into the active turn's pending-input queue so it
/// reaches the next sampling request. Warnings and telemetry are handled in both
/// cases.
pub(crate) async fn drain_async_hook_results(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    delivery: AsyncHookDelivery<'_>,
) {
    while let Ok(result) = sess.async_hook_results.try_recv() {
        if let AsyncHookDelivery::AfterRollback(discarded) = delivery
            && result.run.scope == codex_protocol::protocol::HookScope::Turn
            && result
                .turn_id
                .as_ref()
                .is_some_and(|turn_id| discarded.contains(turn_id))
        {
            continue;
        }
        let additional_contexts = result
            .run
            .entries
            .iter()
            .filter(|entry| entry.kind == HookOutputEntryKind::Context)
            .map(|entry| entry.text.clone())
            .collect::<Vec<_>>();

        if matches!(
            delivery,
            AsyncHookDelivery::BeforeUserPrompt | AsyncHookDelivery::AfterRollback(_)
        ) {
            record_additional_contexts(sess, turn_context, additional_contexts).await;
        } else if !additional_contexts.is_empty() {
            let _ = sess
                .inject_hook_context_if_running(additional_context_messages(additional_contexts))
                .await;
        }

        for entry in &result.run.entries {
            if entry.kind == HookOutputEntryKind::Warning {
                sess.send_event(
                    turn_context,
                    EventMsg::Warning(WarningEvent {
                        message: entry.text.clone(),
                    }),
                )
                .await;
            }
        }

        emit_hook_completed_events(sess, turn_context, vec![result]).await;
    }
}
