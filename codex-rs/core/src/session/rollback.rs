//! Replays a persisted thread rollback and reconciles session state before delivery.

use crate::context::NodeReplReviewEvidence;
use crate::session::session::Session;
use crate::state::ReasoningEffortPin;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_history::RolloutItem;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::WarningEvent;
use std::sync::Arc;

pub(super) async fn handle(sess: &Arc<Session>, sub_id: String, num_turns: u32) {
    if num_turns == 0 {
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                misalignment: None,
                message: "num_turns must be >= 1".to_string(),
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
            }),
        })
        .await;
        return;
    }

    let has_active_turn = { sess.active_turn.lock().await.is_some() };
    if has_active_turn {
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                misalignment: None,
                message: "Cannot rollback while a turn is in progress.".to_string(),
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
            }),
        })
        .await;
        return;
    }

    let turn_context = sess
        .new_turn_with_default_settings(sub_id, Default::default())
        .await;
    let live_thread = match sess.live_thread_for_persistence("rollback thread") {
        Ok(live_thread) => live_thread,
        Err(_) => {
            sess.send_event_raw(Event {
                id: turn_context.sub_id.clone(),
                msg: EventMsg::Error(ErrorEvent {
                    misalignment: None,
                    message: "thread rollback requires persisted thread history".to_string(),
                    codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                }),
            })
            .await;
            return;
        }
    };
    if let Err(err) = live_thread.flush().await {
        sess.send_event_raw(Event {
            id: turn_context.sub_id.clone(),
            msg: EventMsg::Error(ErrorEvent {
                misalignment: None,
                message: format!("failed to flush thread persistence for rollback replay: {err}"),
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
            }),
        })
        .await;
        return;
    }

    let stored_history = match live_thread.load_history(/*include_archived*/ false).await {
        Ok(history) => history,
        Err(err) => {
            sess.send_event_raw(Event {
                id: turn_context.sub_id.clone(),
                msg: EventMsg::Error(ErrorEvent {
                    misalignment: None,
                    message: format!("failed to load thread history for rollback replay: {err}"),
                    codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                }),
            })
            .await;
            return;
        }
    };

    let turn_ids_before_rollback = build_turns_from_rollout_items(&stored_history.items)
        .into_iter()
        .map(|turn| turn.id)
        .collect::<std::collections::HashSet<_>>();
    let rollback_event = ThreadRolledBackEvent { num_turns };
    let rollback_msg = EventMsg::ThreadRolledBack(rollback_event.clone());
    let replay_items = stored_history
        .items
        .into_iter()
        .chain(std::iter::once(RolloutItem::EventMsg(rollback_msg.clone())))
        .collect::<Vec<_>>();
    let retained_turn_ids = build_turns_from_rollout_items(&replay_items)
        .into_iter()
        .map(|turn| turn.id)
        .collect::<std::collections::HashSet<_>>();
    let discarded_turn_ids = turn_ids_before_rollback
        .difference(&retained_turn_ids)
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    sess.apply_rollout_reconstruction(turn_context.as_ref(), replay_items.as_slice())
        .await;
    {
        let mut state = sess.state.lock().await;
        // Keep the baseline while startup prewarm is retained for the first turn,
        // including when its task has not established the pin yet.
        if state.startup_prewarm.is_none() {
            state.reasoning_effort_pin = ReasoningEffortPin::Unset;
        }
    }
    // Persist the cut before any surviving async-hook context. Replay applies the rollback marker
    // to the earlier suffix, then retains context recorded after that marker.
    sess.persist_rollout_items(&[RolloutItem::EventMsg(rollback_msg.clone())])
        .await;
    sess.hooks().abort_turns(&discarded_turn_ids).await;
    crate::hook_runtime::drain_async_hook_results(
        sess,
        &turn_context,
        crate::hook_runtime::AsyncHookDelivery::AfterRollback(&discarded_turn_ids),
    )
    .await;
    sess.services
        .thread_extension_data
        .remove::<NodeReplReviewEvidence>();
    sess.guardian_review_session().invalidate().await;
    sess.services
        .agent_control
        .rollout_budget()
        .rearm_reminder(sess.thread_id());
    sess.recompute_token_usage(turn_context.as_ref()).await;

    if let Err(err) = sess.flush_rollout().await {
        sess.send_event(
            turn_context.as_ref(),
            EventMsg::Warning(WarningEvent {
                message: format!(
                    "Rolled the thread back, but failed to save the rollback marker. Codex will continue retrying. Error: {err}"
                ),
            }),
        )
        .await;
    }

    sess.deliver_event_raw(Event {
        id: turn_context.sub_id.clone(),
        msg: rollback_msg,
    })
    .await;
}
