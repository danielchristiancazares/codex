use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ToolInvocationFuture;
use codex_protocol::ResponseItemId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use serde_json::Value as JsonValue;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::submit_nested_tool;
use crate::session::step_context::StepContext;
use crate::tools::ExecutedToolCallRecorder;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::parallel::ToolCallRuntime;

pub(super) struct CodeModeDispatchBroker {
    dispatch_gates: Arc<Mutex<HashMap<CellId, CellDispatchGate>>>,
    turn_workers: Arc<Mutex<HashMap<String, TurnDispatchWorker>>>,
    next_worker_generation: AtomicU64,
    executed_tool_calls: Option<Arc<ExecutedToolCallRecorder>>,
}

struct CellDispatchGate {
    owner: watch::Sender<Option<CellOwner>>,
    // Keep the original exec item when later waits resume this cell.
    originating_item_id: Option<ResponseItemId>,
}

#[derive(Clone)]
struct CellOwner {
    turn_id: String,
    session: Weak<crate::session::session::Session>,
}

#[derive(Clone)]
struct TurnDispatchWorker {
    generation: u64,
    dispatch_tx: async_channel::Sender<DispatchMessage>,
}

impl CodeModeDispatchBroker {
    pub(super) fn new(executed_tool_calls: Option<Arc<ExecutedToolCallRecorder>>) -> Self {
        Self {
            dispatch_gates: Arc::new(Mutex::new(HashMap::new())),
            turn_workers: Arc::new(Mutex::new(HashMap::new())),
            next_worker_generation: AtomicU64::new(0),
            executed_tool_calls,
        }
    }

    pub(super) fn mark_cell_ready_for_dispatch(
        &self,
        cell_id: &CellId,
        session: &Arc<crate::session::session::Session>,
        turn_id: &str,
        originating_item_id: Option<ResponseItemId>,
    ) -> Result<(), String> {
        let owner = {
            let mut dispatch_gates = self
                .dispatch_gates
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let gate = dispatch_gates
                .entry(cell_id.clone())
                .or_insert_with(|| CellDispatchGate {
                    owner: watch::channel(None).0,
                    originating_item_id: None,
                });
            if let Some(existing_owner) = gate.owner.borrow().as_ref()
                && existing_owner.turn_id != turn_id
            {
                return Err(format!(
                    "code mode cell {cell_id} is already owned by turn {}",
                    existing_owner.turn_id
                ));
            }
            gate.originating_item_id = originating_item_id;
            gate.owner.clone()
        };
        owner.send_replace(Some(CellOwner {
            turn_id: turn_id.to_string(),
            session: Arc::downgrade(session),
        }));
        Ok(())
    }

    pub(super) fn cell_originating_item_id(&self, cell_id: &CellId) -> Option<ResponseItemId> {
        self.dispatch_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(cell_id)
            .and_then(|gate| gate.originating_item_id.clone())
    }

    pub(super) fn close_cell(&self, cell_id: &CellId) {
        let mut dispatch_gates = self
            .dispatch_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        dispatch_gates.remove(cell_id);
        if let Some(recorder) = &self.executed_tool_calls {
            recorder.finish_cell_recording(cell_id);
        }
    }

    pub(super) fn active_cell_ids_for_turn(&self, turn_id: &str) -> Vec<CellId> {
        self.dispatch_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|&(_cell_id, gate)| {
                gate.owner
                    .borrow()
                    .as_ref()
                    .is_some_and(|owner| owner.turn_id == turn_id)
            })
            .map(|(cell_id, _gate)| cell_id.clone())
            .collect()
    }

    pub(super) fn start_turn_worker(
        &self,
        exec: ExecContext,
        step_context: Arc<StepContext>,
        tracker: SharedTurnDiffTracker,
    ) -> CodeModeDispatchWorker {
        let track_completeness = exec
            .turn
            .config
            .features
            .enabled(codex_features::Feature::ExecutedToolCallMetadata);
        let tool_runtime = ToolCallRuntime::new(Arc::clone(&exec.session), step_context, tracker);
        let host = Arc::new(CoreTurnHost { exec, tool_runtime });
        let turn_id = host.exec.turn.sub_id.clone();
        let generation = self.next_worker_generation.fetch_add(1, Ordering::Relaxed);
        let (dispatch_tx, dispatch_rx) = async_channel::unbounded();
        self.turn_workers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                turn_id.clone(),
                TurnDispatchWorker {
                    generation,
                    dispatch_tx,
                },
            );
        let dispatch_gates = Arc::clone(&self.dispatch_gates);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                let message = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    message = dispatch_rx.recv() => message.ok(),
                };
                let Some(message) = message else {
                    break;
                };
                match message {
                    DispatchMessage::Notify {
                        call_id,
                        cell_id,
                        text,
                        cancellation_token,
                        response_tx,
                    } => {
                        let response = if cancellation_token.is_cancelled() {
                            Err("code mode notification cancelled".to_string())
                        } else {
                            host.notify(call_id, cell_id, text).await
                        };
                        let _ = response_tx.send(response);
                    }
                    DispatchMessage::InvokeTool {
                        invocation,
                        cancellation_token,
                        response_tx,
                        span,
                    } => {
                        let cell_id = invocation.cell_id.clone();
                        let host = Arc::clone(&host);
                        let dispatch_gates = Arc::clone(&dispatch_gates);
                        tokio::spawn(async move {
                            let invocation = {
                                let dispatch_gate = track_completeness.then(|| {
                                    dispatch_gates
                                        .lock()
                                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                                });
                                if dispatch_gate.as_ref().is_some_and(|gates| {
                                    cancellation_token.is_cancelled()
                                        || !gates.contains_key(&cell_id)
                                }) {
                                    return;
                                }
                                // Submission and cell closure share this gate.
                                span.in_scope(|| {
                                    host.submit_tool(invocation, cancellation_token.clone())
                                })
                                .instrument(span)
                            };
                            tokio::pin!(invocation);
                            let response = tokio::select! {
                                biased;
                                _ = cancellation_token.cancelled() => invocation.await,
                                response = &mut invocation => response,
                            };
                            let _ = response_tx.send(response);
                        });
                    }
                }
            }
        });
        CodeModeDispatchWorker {
            shutdown_tx: Some(shutdown_tx),
            turn_workers: Arc::clone(&self.turn_workers),
            turn_id,
            generation,
        }
    }

    async fn dispatch_sender_for_owner(
        &self,
        owner: &CellOwner,
    ) -> Result<async_channel::Sender<DispatchMessage>, String> {
        let Some(session) = owner.session.upgrade() else {
            return Err("code mode cell owner session is unavailable".to_string());
        };
        if !session.is_turn_running(owner.turn_id.as_str()).await {
            return Err(format!(
                "code mode cell owner turn {} is no longer active",
                owner.turn_id
            ));
        }
        self.turn_workers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&owner.turn_id)
            .map(|worker| worker.dispatch_tx.clone())
            .ok_or_else(|| {
                format!(
                    "code mode dispatcher for owner turn {} is unavailable",
                    owner.turn_id
                )
            })
    }
}

fn dispatch_gate(
    dispatch_gates: &Mutex<HashMap<CellId, CellDispatchGate>>,
    cell_id: &CellId,
) -> watch::Sender<Option<CellOwner>> {
    let mut dispatch_gates = match dispatch_gates.lock() {
        Ok(dispatch_gates) => dispatch_gates,
        Err(poisoned) => poisoned.into_inner(),
    };
    dispatch_gates
        .entry(cell_id.clone())
        .or_insert_with(|| CellDispatchGate {
            owner: watch::channel(None).0,
            originating_item_id: None,
        })
        .owner
        .clone()
}

fn remove_dispatch_gate(
    dispatch_gates: &Mutex<HashMap<CellId, CellDispatchGate>>,
    cell_id: &CellId,
) {
    let mut dispatch_gates = match dispatch_gates.lock() {
        Ok(dispatch_gates) => dispatch_gates,
        Err(poisoned) => poisoned.into_inner(),
    };
    dispatch_gates.remove(cell_id);
}

async fn wait_until_cell_ready_for_dispatch(
    dispatch_gates: &Mutex<HashMap<CellId, CellDispatchGate>>,
    cell_id: &CellId,
    cancellation_token: &CancellationToken,
) -> Option<CellOwner> {
    if cancellation_token.is_cancelled() {
        return None;
    }
    let mut owner_rx = dispatch_gate(dispatch_gates, cell_id).subscribe();
    loop {
        if let Some(owner) = owner_rx.borrow_and_update().clone() {
            return Some(owner);
        }
        tokio::select! {
            changed = owner_rx.changed() => {
                if changed.is_err() {
                    return None;
                }
            }
            _ = cancellation_token.cancelled() => return None,
        }
    }
}

impl CodeModeSessionDelegate for CodeModeDispatchBroker {
    #[tracing::instrument(
        name = "code_mode.broker.invoke_tool",
        level = "info",
        skip_all,
        fields(
            cell.id = %invocation.cell_id,
            runtime_tool_call_id = invocation.runtime_tool_call_id.as_str(),
            tool_name = invocation.tool_name.name.as_str(),
            tool_namespace = invocation.tool_name.namespace.as_deref(),
        )
    )]
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            if cancellation_token.is_cancelled() {
                return Err("code mode nested tool call cancelled".to_string());
            }
            let cell_id = invocation.cell_id.clone();
            let Some(owner) = wait_until_cell_ready_for_dispatch(
                &self.dispatch_gates,
                &cell_id,
                &cancellation_token,
            )
            .await
            else {
                remove_dispatch_gate(&self.dispatch_gates, &cell_id);
                return Err("code mode nested tool call cancelled".to_string());
            };
            let dispatch_tx = self.dispatch_sender_for_owner(&owner).await?;
            let (response_tx, response_rx) = oneshot::channel();
            dispatch_tx
                .send(DispatchMessage::InvokeTool {
                    invocation,
                    cancellation_token: cancellation_token.clone(),
                    response_tx,
                    span: tracing::Span::current(),
                })
                .await
                .map_err(|_| "code mode nested tool dispatcher is unavailable".to_string())?;
            tokio::select! {
                response = response_rx => response
                    .map_err(|_| "code mode nested tool dispatcher stopped".to_string())?,
                _ = cancellation_token.cancelled() => {
                    Err("code mode nested tool call cancelled".to_string())
                }
            }
        })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            if cancellation_token.is_cancelled() {
                return Err("code mode notification cancelled".to_string());
            }
            let Some(owner) = wait_until_cell_ready_for_dispatch(
                &self.dispatch_gates,
                &cell_id,
                &cancellation_token,
            )
            .await
            else {
                remove_dispatch_gate(&self.dispatch_gates, &cell_id);
                return Err("code mode notification cancelled".to_string());
            };
            let dispatch_tx = self.dispatch_sender_for_owner(&owner).await?;
            let (response_tx, response_rx) = oneshot::channel();
            dispatch_tx
                .send(DispatchMessage::Notify {
                    call_id,
                    cell_id,
                    text,
                    cancellation_token: cancellation_token.clone(),
                    response_tx,
                })
                .await
                .map_err(|_| "code mode notification dispatcher is unavailable".to_string())?;
            tokio::select! {
                response = response_rx => response
                    .map_err(|_| "code mode notification dispatcher stopped".to_string())?,
                _ = cancellation_token.cancelled() => {
                    Err("code mode notification cancelled".to_string())
                }
            }
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.close_cell(cell_id);
    }
}

enum DispatchMessage {
    InvokeTool {
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
        response_tx: oneshot::Sender<Result<JsonValue, String>>,
        span: tracing::Span,
    },
    Notify {
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

pub(crate) struct CodeModeDispatchWorker {
    shutdown_tx: Option<oneshot::Sender<()>>,
    turn_workers: Arc<Mutex<HashMap<String, TurnDispatchWorker>>>,
    turn_id: String,
    generation: u64,
}

impl Drop for CodeModeDispatchWorker {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        let mut turn_workers = self
            .turn_workers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if turn_workers
            .get(&self.turn_id)
            .is_some_and(|worker| worker.generation == self.generation)
        {
            turn_workers.remove(&self.turn_id);
        }
    }
}

struct CoreTurnHost {
    exec: ExecContext,
    tool_runtime: ToolCallRuntime,
}

impl CoreTurnHost {
    fn submit_tool(
        &self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> impl std::future::Future<Output = Result<JsonValue, String>> + Send + 'static {
        let invocation = submit_nested_tool(
            self.exec.clone(),
            self.tool_runtime.clone(),
            invocation,
            cancellation_token,
        )
        .map_err(|error| error.to_string());
        async move { invocation?.await.map_err(|error| error.to_string()) }
    }

    async fn notify(&self, call_id: String, cell_id: CellId, text: String) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        self.exec
            .session
            .inject_if_turn_running(
                self.exec.turn.sub_id.as_str(),
                vec![ResponseItem::CustomToolCallOutput {
                    id: None,
                    call_id,
                    name: Some(PUBLIC_TOOL_NAME.to_string()),
                    output: FunctionCallOutputPayload::from_text(text),
                    internal_chat_message_metadata_passthrough: None,
                }],
            )
            .await
            .map_err(|_| {
                format!("failed to inject exec notify message for cell {cell_id}: no active turn")
            })
    }
}
