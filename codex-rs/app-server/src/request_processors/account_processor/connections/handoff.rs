//! Owns the live credential handoff and its connection-scoped recovery receipt.

use super::*;
use crate::outgoing_message::ConnectionId;

pub(in super::super) struct ConnectionSwitches {
    state: SwitchState,
}

enum SwitchState {
    AcceptSelections,
    FinishHandoff {
        request: SwitchRequest,
        activation: Box<ActivatedConnection>,
    },
    RetainSettlement {
        request: SwitchRequest,
        settlement: Settlement,
    },
}

struct SwitchRequest {
    owner: ConnectionId,
    id: ConnectionText,
    target: ConnectionText,
}

#[derive(Clone)]
pub(super) enum Settlement {
    Commit,
    Restore,
}

impl ConnectionSwitches {
    pub(in super::super) fn new() -> Self {
        Self {
            state: SwitchState::AcceptSelections,
        }
    }

    pub(in super::super) fn require_ready(&self) -> Result<(), JSONRPCErrorError> {
        match self.state {
            SwitchState::AcceptSelections | SwitchState::RetainSettlement { .. } => Ok(()),
            SwitchState::FinishHandoff { .. } => {
                Err(invalid_request("An account switch is already in progress."))
            }
        }
    }

    pub(super) async fn select(
        &mut self,
        processor: &AccountRequestProcessor,
        owner: ConnectionId,
        id: ConnectionText,
        target: ConnectionText,
    ) -> Result<(), JSONRPCErrorError> {
        match &self.state {
            SwitchState::FinishHandoff { request, .. }
                if request.owner == owner && request.id == id && request.target == target =>
            {
                return Ok(());
            }
            SwitchState::RetainSettlement { request, .. }
                if request.owner == owner && request.id == id =>
            {
                return Err(invalid_request(
                    "This switch receipt has already settled; start a new switch.",
                ));
            }
            SwitchState::AcceptSelections | SwitchState::RetainSettlement { .. } => {}
            SwitchState::FinishHandoff { .. } => {
                return Err(invalid_request("An account switch is already in progress."));
            }
        }
        for thread_id in processor.thread_manager.list_thread_ids().await {
            let thread = processor
                .thread_manager
                .get_thread(thread_id)
                .await
                .map_err(|error| internal_error(error.to_string()))?;
            match thread.agent_status().await {
                AgentStatus::Running => {
                    return Err(invalid_request(
                        "Wait for running agents before switching accounts.",
                    ));
                }
                AgentStatus::PendingInit
                | AgentStatus::Interrupted
                | AgentStatus::Completed(_)
                | AgentStatus::Errored(_)
                | AgentStatus::Shutdown
                | AgentStatus::NotFound => {}
            }
            if !thread.list_background_terminals().await.is_empty() {
                return Err(invalid_request(
                    "Stop background terminals before switching accounts.",
                ));
            }
        }
        processor.cancel_active_login().await;
        let prepared = processor
            .auth_manager
            .prepare_saved_connection(target.as_str())
            .await
            .map_err(connection_error)?;
        let activation = processor
            .auth_manager
            .activate_saved_connection(prepared)
            .await
            .map_err(connection_error)?;
        self.state = SwitchState::FinishHandoff {
            request: SwitchRequest { owner, id, target },
            activation: Box::new(activation),
        };
        processor.notify_connection_changed().await;
        Ok(())
    }

    pub(super) async fn settle(
        &mut self,
        processor: &AccountRequestProcessor,
        owner: ConnectionId,
        id: ConnectionText,
        settlement: Settlement,
    ) -> Result<(), JSONRPCErrorError> {
        match &self.state {
            SwitchState::RetainSettlement {
                request,
                settlement: completed,
            } if request.owner == owner && request.id == id => {
                return match (completed, &settlement) {
                    (Settlement::Commit, Settlement::Commit) => {
                        let store = processor.auth_manager.connection_store();
                        let connection = store
                            .resolve(request.target.as_str())
                            .map_err(connection_error)?;
                        store.remember(&connection).map_err(connection_error)
                    }
                    (Settlement::Restore, Settlement::Restore) => Ok(()),
                    (Settlement::Commit, Settlement::Restore)
                    | (Settlement::Restore, Settlement::Commit) => Err(invalid_request(
                        "This account switch already settled with a different outcome.",
                    )),
                };
            }
            SwitchState::FinishHandoff { request, .. }
                if request.owner == owner && request.id == id => {}
            SwitchState::AcceptSelections
            | SwitchState::RetainSettlement { .. }
            | SwitchState::FinishHandoff { .. } => {
                return Err(invalid_request(
                    "This account switch receipt is unavailable for this connection.",
                ));
            }
        }
        match std::mem::replace(&mut self.state, SwitchState::AcceptSelections) {
            SwitchState::FinishHandoff {
                request,
                activation,
            } => {
                self.state = SwitchState::RetainSettlement {
                    request,
                    settlement: settlement.clone(),
                };
                match settlement {
                    Settlement::Commit => {
                        activation.commit().map_err(connection_error)?;
                    }
                    Settlement::Restore => {
                        activation.restore().await.map_err(connection_error)?;
                        processor.notify_connection_changed().await;
                    }
                }
                Ok(())
            }
            SwitchState::AcceptSelections | SwitchState::RetainSettlement { .. } => {
                Err(internal_error("Account handoff ownership was lost."))
            }
        }
    }

    pub(super) async fn disconnected(
        &mut self,
        processor: &AccountRequestProcessor,
        owner: ConnectionId,
    ) {
        match &self.state {
            SwitchState::FinishHandoff { request, .. } if request.owner == owner => {
                let id = request.id.clone();
                if let Err(error) = self.settle(processor, owner, id, Settlement::Restore).await {
                    tracing::warn!(message = %error.message, "account handoff recovery failed after disconnect");
                }
            }
            SwitchState::AcceptSelections
            | SwitchState::FinishHandoff { .. }
            | SwitchState::RetainSettlement { .. } => {}
        }
    }
}
