//! Runtime-owned suspension and admission for coordination tools.

use crate::function_tool::FunctionCallError;
use crate::session::InputQueueActivity;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use serde::Deserialize;
use std::num::NonZeroU64;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLockWriteGuard;
use tokio::sync::watch;

#[derive(Debug, Default)]
pub(crate) enum WaitPolicy {
    #[default]
    UntilActivity,
    PollAvailable,
    DeadlineAfter(NonZeroU64),
}

impl<'de> Deserialize<'de> for WaitPolicy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match NonZeroU64::new(u64::deserialize(deserializer)?) {
            Some(milliseconds) => Ok(Self::DeadlineAfter(milliseconds)),
            None => Ok(Self::PollAvailable),
        }
    }
}

impl WaitPolicy {
    pub(crate) async fn wait_for_cell(
        &self,
        session: &Session,
        turn: &TurnContext,
        cell_id: codex_code_mode::CellId,
    ) -> Result<codex_code_mode::WaitOutcome, FunctionCallError> {
        let activity = InputWakeup::subscribe(session, turn).await.wait();
        tokio::pin!(activity);
        loop {
            let response = tokio::select! {
                biased;
                response = session.services.code_mode_service.wait(codex_code_mode::WaitRequest {
                    cell_id: cell_id.clone(),
                    yield_time_ms: self.poll_interval_ms(codex_code_mode::DEFAULT_WAIT_YIELD_TIME_MS),
                }) => response.map_err(FunctionCallError::RespondToModel)?,
                activity = &mut activity => {
                    activity?;
                    return Err(FunctionCallError::RespondToModel(format!(
                        "Wait interrupted by new input. Code cell {cell_id} remains available."
                    )));
                }
            };
            match (self, &response) {
                (
                    Self::UntilActivity,
                    codex_code_mode::WaitOutcome::LiveCell(
                        codex_code_mode::RuntimeResponse::Yielded { content_items, .. },
                    ),
                ) if content_items.is_empty() => {}
                (Self::UntilActivity | Self::PollAvailable | Self::DeadlineAfter(_), _) => {
                    return Ok(response);
                }
            }
        }
    }

    pub(crate) fn validate_maximum(&self, maximum_ms: u64) -> Result<(), FunctionCallError> {
        match self {
            Self::DeadlineAfter(milliseconds) if milliseconds.get() > maximum_ms => {
                Err(FunctionCallError::RespondToModel(format!(
                    "timeout_ms must be at most {maximum_ms}"
                )))
            }
            Self::UntilActivity | Self::PollAvailable | Self::DeadlineAfter(_) => Ok(()),
        }
    }

    pub(crate) fn require_positive_deadline(&self) -> Result<(), FunctionCallError> {
        match self {
            Self::PollAvailable => Err(FunctionCallError::RespondToModel(
                "timeout_ms must be greater than zero".into(),
            )),
            Self::UntilActivity | Self::DeadlineAfter(_) => Ok(()),
        }
    }

    pub(crate) fn describe_clamping(&self, message: &str, minimum_ms: u64) -> String {
        match self {
            Self::DeadlineAfter(milliseconds) if milliseconds.get() < minimum_ms => format!(
                "{message}\n\nRequested timeout of {milliseconds}ms was clamped to the minimum of {minimum_ms}ms."
            ),
            Self::PollAvailable if minimum_ms > 0 => format!(
                "{message}\n\nRequested timeout of 0ms was clamped to the minimum of {minimum_ms}ms."
            ),
            Self::UntilActivity | Self::PollAvailable | Self::DeadlineAfter(_) => {
                message.to_owned()
            }
        }
    }

    pub(crate) async fn expired(&self, minimum_ms: u64, maximum_ms: u64) {
        match self {
            Self::UntilActivity => std::future::pending().await,
            Self::PollAvailable => tokio::time::sleep(Duration::from_millis(minimum_ms)).await,
            Self::DeadlineAfter(milliseconds) => {
                tokio::time::sleep(Duration::from_millis(
                    milliseconds.get().clamp(minimum_ms, maximum_ms),
                ))
                .await;
            }
        }
    }

    pub(crate) fn poll_interval_ms(&self, default_ms: u64) -> u64 {
        match self {
            Self::UntilActivity => default_ms,
            Self::PollAvailable => 0,
            Self::DeadlineAfter(milliseconds) => milliseconds.get(),
        }
    }

    pub(crate) async fn terminal_poll(
        &self,
        session: &Session,
        turn: &TurnContext,
        process_id: i32,
        input: &str,
    ) -> Result<crate::unified_exec::PollCollection, FunctionCallError> {
        match (self, input.is_empty()) {
            (Self::UntilActivity, true) => {
                let activity = InputWakeup::subscribe(session, turn).await;
                tokio::select! {
                    ready = session.services.unified_exec_manager.wait_for_output(process_id) => {
                        ready.map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
                        Ok(crate::unified_exec::PollCollection::Available)
                    }
                    activity = activity.wait() => {
                        activity?;
                        Err(FunctionCallError::RespondToModel(format!(
                            "Wait interrupted by new input. Terminal session {process_id} remains available."
                        )))
                    }
                }
            }
            (Self::UntilActivity, false) | (Self::PollAvailable | Self::DeadlineAfter(_), _) => {
                Ok(crate::unified_exec::PollCollection::UntilDeadline)
            }
        }
    }
}

pub(crate) enum InputWakeup {
    DeliverQueued(InputQueueActivity),
    AwaitQueued(watch::Receiver<InputQueueActivity>),
}

impl InputWakeup {
    pub(crate) async fn subscribe(session: &Session, turn: &TurnContext) -> Self {
        match session
            .input_queue
            .subscribe_activity(
                session
                    .input_queue
                    .turn_state_for_sub_id(&session.active_turn, &turn.sub_id)
                    .await
                    .as_deref(),
            )
            .await
        {
            (_, Some(activity)) => Self::DeliverQueued(activity),
            (receiver, None) => Self::AwaitQueued(receiver),
        }
    }

    pub(crate) async fn wait(self) -> Result<InputQueueActivity, FunctionCallError> {
        match self {
            Self::DeliverQueued(activity) => Ok(activity),
            Self::AwaitQueued(mut receiver) => {
                receiver.changed().await.map_err(|_| {
                    FunctionCallError::RespondToModel("The session input queue closed.".into())
                })?;
                Ok(*receiver.borrow_and_update())
            }
        }
    }
}

pub(crate) enum ExecutionScope {
    Serialized,
    Coordination,
}

pub(crate) enum ExecutionGuard<'a> {
    Coordination,
    Shared { _guard: RwLockReadGuard<'a, ()> },
    Exclusive { _guard: RwLockWriteGuard<'a, ()> },
}

impl ExecutionScope {
    pub(crate) async fn acquire(
        self,
        lock: &RwLock<()>,
        parallel: ParallelExecution,
    ) -> ExecutionGuard<'_> {
        match self {
            Self::Coordination => ExecutionGuard::Coordination,
            Self::Serialized => match parallel {
                ParallelExecution::Shared => ExecutionGuard::Shared {
                    _guard: lock.read().await,
                },
                ParallelExecution::Exclusive => ExecutionGuard::Exclusive {
                    _guard: lock.write().await,
                },
            },
        }
    }
}

pub(crate) enum ParallelExecution {
    Shared,
    Exclusive,
}

#[cfg(test)]
#[path = "runtime_wait_tests.rs"]
mod tests;
