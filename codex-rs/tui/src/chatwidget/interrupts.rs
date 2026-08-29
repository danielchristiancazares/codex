//! Queue prompt overlays and deferred tool activity while another interrupt is visible.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;

use crate::app::app_server_requests::ResolvedAppServerRequest;
use crate::approval_events::ApplyPatchApprovalRequestEvent;
use crate::approval_events::ExecApprovalRequestEvent;
use crate::diff_model::FileChange;
use crate::history_cell::PlainHistoryCell;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::WebSearchAction;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;

use super::ChatWidget;

#[derive(Debug)]
pub(crate) enum QueuedInterrupt {
    ExecApproval(ExecApprovalRequestEvent),
    ApplyPatchApproval(ApplyPatchApprovalRequestEvent),
    Elicitation {
        request_id: AppServerRequestId,
        params: McpServerElicitationRequestParams,
    },
    RequestPermissions(RequestPermissionsEvent),
    RequestUserInput(ToolRequestUserInputParams),
    ItemStarted(ThreadItem),
    ItemCompleted(ThreadItem),
    PatchApplyBegin(HashMap<PathBuf, FileChange>),
    ViewImage(LegacyAppPathString),
    ImageGenerationBegin,
    ImageGenerationEnd {
        call_id: String,
        status: String,
        revised_prompt: Option<String>,
        saved_path: Option<AbsolutePathBuf>,
    },
    WebSearchBegin(String),
    WebSearchEnd {
        call_id: String,
        query: String,
        action: WebSearchAction,
    },
    CollabEvent(PlainHistoryCell),
}

#[derive(Default)]
pub(crate) struct InterruptManager {
    queue: VecDeque<QueuedInterrupt>,
}

impl InterruptManager {
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Excludes lifecycle events that never claim protected interactive input.
    pub(crate) fn has_pending_prompt(&self) -> bool {
        self.queue.iter().any(|interrupt| {
            matches!(
                interrupt,
                QueuedInterrupt::ExecApproval(_)
                    | QueuedInterrupt::ApplyPatchApproval(_)
                    | QueuedInterrupt::Elicitation { .. }
                    | QueuedInterrupt::RequestPermissions(_)
                    | QueuedInterrupt::RequestUserInput(_)
            )
        })
    }

    pub(crate) fn push_exec_approval(&mut self, ev: ExecApprovalRequestEvent) {
        self.queue.push_back(QueuedInterrupt::ExecApproval(ev));
    }

    pub(crate) fn push_apply_patch_approval(&mut self, ev: ApplyPatchApprovalRequestEvent) {
        self.queue
            .push_back(QueuedInterrupt::ApplyPatchApproval(ev));
    }

    pub(crate) fn push_elicitation(
        &mut self,
        request_id: AppServerRequestId,
        params: McpServerElicitationRequestParams,
    ) {
        self.queue
            .push_back(QueuedInterrupt::Elicitation { request_id, params });
    }

    pub(crate) fn push_request_permissions(&mut self, ev: RequestPermissionsEvent) {
        self.queue
            .push_back(QueuedInterrupt::RequestPermissions(ev));
    }

    pub(crate) fn push_user_input(&mut self, ev: ToolRequestUserInputParams) {
        self.queue.push_back(QueuedInterrupt::RequestUserInput(ev));
    }

    pub(crate) fn push_item_started(&mut self, item: ThreadItem) {
        self.queue.push_back(QueuedInterrupt::ItemStarted(item));
    }

    pub(crate) fn push_item_completed(&mut self, item: ThreadItem) {
        self.queue.push_back(QueuedInterrupt::ItemCompleted(item));
    }

    pub(crate) fn push_patch_apply_begin(&mut self, changes: HashMap<PathBuf, FileChange>) {
        self.queue
            .push_back(QueuedInterrupt::PatchApplyBegin(changes));
    }

    pub(crate) fn push_view_image(&mut self, path: LegacyAppPathString) {
        self.queue.push_back(QueuedInterrupt::ViewImage(path));
    }

    pub(crate) fn push_image_generation_begin(&mut self) {
        self.queue.push_back(QueuedInterrupt::ImageGenerationBegin);
    }

    pub(crate) fn push_image_generation_end(
        &mut self,
        call_id: String,
        status: String,
        revised_prompt: Option<String>,
        saved_path: Option<AbsolutePathBuf>,
    ) {
        self.queue.push_back(QueuedInterrupt::ImageGenerationEnd {
            call_id,
            status,
            revised_prompt,
            saved_path,
        });
    }

    pub(crate) fn push_web_search_begin(&mut self, call_id: String) {
        self.queue
            .push_back(QueuedInterrupt::WebSearchBegin(call_id));
    }

    pub(crate) fn push_web_search_end(
        &mut self,
        call_id: String,
        query: String,
        action: WebSearchAction,
    ) {
        self.queue.push_back(QueuedInterrupt::WebSearchEnd {
            call_id,
            query,
            action,
        });
    }

    pub(crate) fn push_collab_event(&mut self, cell: PlainHistoryCell) {
        self.queue.push_back(QueuedInterrupt::CollabEvent(cell));
    }

    pub(crate) fn remove_resolved_prompt(&mut self, request: &ResolvedAppServerRequest) -> bool {
        if !self.has_pending_prompt() {
            return false;
        }
        let original_len = self.queue.len();
        self.queue
            .retain(|queued| !queued.matches_resolved_prompt(request));
        self.queue.len() != original_len
    }

    pub(crate) fn pop_front(&mut self) -> Option<QueuedInterrupt> {
        self.queue.pop_front()
    }

    pub(crate) fn push_front(&mut self, interrupt: QueuedInterrupt) {
        self.queue.push_front(interrupt);
    }
}

impl QueuedInterrupt {
    pub(crate) fn handle_now(self, chat: &mut ChatWidget) {
        match self {
            QueuedInterrupt::ExecApproval(ev) => chat.handle_exec_approval_now(ev),
            QueuedInterrupt::ApplyPatchApproval(ev) => chat.handle_apply_patch_approval_now(ev),
            QueuedInterrupt::Elicitation { request_id, params } => {
                chat.handle_elicitation_request_now(request_id, params);
            }
            QueuedInterrupt::RequestPermissions(ev) => chat.handle_request_permissions_now(ev),
            QueuedInterrupt::RequestUserInput(ev) => chat.handle_request_user_input_now(ev),
            QueuedInterrupt::ItemStarted(item) => chat.handle_queued_item_started_now(item),
            QueuedInterrupt::ItemCompleted(item) => chat.handle_queued_item_completed_now(item),
            QueuedInterrupt::PatchApplyBegin(changes) => {
                chat.handle_patch_apply_begin_now(changes);
            }
            QueuedInterrupt::ViewImage(path) => chat.handle_view_image_tool_call_now(path),
            QueuedInterrupt::ImageGenerationBegin => chat.handle_image_generation_begin_now(),
            QueuedInterrupt::ImageGenerationEnd {
                call_id,
                status,
                revised_prompt,
                saved_path,
            } => chat.handle_image_generation_end_now(call_id, status, revised_prompt, saved_path),
            QueuedInterrupt::WebSearchBegin(call_id) => {
                chat.handle_web_search_begin_now(call_id);
            }
            QueuedInterrupt::WebSearchEnd {
                call_id,
                query,
                action,
            } => chat.handle_web_search_end_now(call_id, query, action),
            QueuedInterrupt::CollabEvent(cell) => chat.handle_collab_event_now(cell),
        }
    }

    pub(crate) fn is_mcp_start(&self) -> bool {
        matches!(
            self,
            QueuedInterrupt::ItemStarted(ThreadItem::McpToolCall { .. })
        )
    }

    pub(crate) fn mcp_completion_call_id(&self) -> Option<&str> {
        match self {
            QueuedInterrupt::ItemCompleted(ThreadItem::McpToolCall { id, .. }) => Some(id),
            QueuedInterrupt::ExecApproval(_)
            | QueuedInterrupt::ApplyPatchApproval(_)
            | QueuedInterrupt::Elicitation { .. }
            | QueuedInterrupt::RequestPermissions(_)
            | QueuedInterrupt::RequestUserInput(_)
            | QueuedInterrupt::ItemStarted(_)
            | QueuedInterrupt::ItemCompleted(_)
            | QueuedInterrupt::PatchApplyBegin(_)
            | QueuedInterrupt::ViewImage(_)
            | QueuedInterrupt::ImageGenerationBegin
            | QueuedInterrupt::ImageGenerationEnd { .. }
            | QueuedInterrupt::WebSearchBegin(_)
            | QueuedInterrupt::WebSearchEnd { .. }
            | QueuedInterrupt::CollabEvent(_) => None,
        }
    }

    fn matches_resolved_prompt(&self, request: &ResolvedAppServerRequest) -> bool {
        match self {
            QueuedInterrupt::ExecApproval(ev) => {
                matches!(request, ResolvedAppServerRequest::ExecApproval { id, .. }
                    if ev.effective_approval_id() == id.as_str())
            }
            QueuedInterrupt::ApplyPatchApproval(ev) => {
                matches!(request, ResolvedAppServerRequest::FileChangeApproval { id, .. }
                    if ev.call_id == id.as_str())
            }
            QueuedInterrupt::Elicitation { request_id, params } => {
                matches!(request, ResolvedAppServerRequest::McpElicitation {
                    server_name,
                    request_id: resolved_request_id,
                } if params.server_name == server_name.as_str() && request_id == resolved_request_id)
            }
            QueuedInterrupt::RequestPermissions(ev) => {
                matches!(request, ResolvedAppServerRequest::PermissionsApproval { id, .. }
                    if ev.call_id == id.as_str())
            }
            QueuedInterrupt::RequestUserInput(ev) => {
                matches!(request, ResolvedAppServerRequest::UserInput { call_id }
                    if ev.item_id == call_id.as_str())
            }
            QueuedInterrupt::ItemStarted(_)
            | QueuedInterrupt::ItemCompleted(_)
            | QueuedInterrupt::PatchApplyBegin(_)
            | QueuedInterrupt::ViewImage(_)
            | QueuedInterrupt::ImageGenerationBegin
            | QueuedInterrupt::ImageGenerationEnd { .. }
            | QueuedInterrupt::WebSearchBegin(_)
            | QueuedInterrupt::WebSearchEnd { .. }
            | QueuedInterrupt::CollabEvent(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::approval_events::ExecApprovalRequestEvent;
    use codex_app_server_protocol::CommandExecutionSource;
    use codex_app_server_protocol::CommandExecutionStatus;
    use codex_app_server_protocol::ThreadItem;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use super::*;

    fn user_input(call_id: &str, turn_id: &str) -> ToolRequestUserInputParams {
        ToolRequestUserInputParams {
            thread_id: "thread-1".to_string(),
            item_id: call_id.to_string(),
            turn_id: turn_id.to_string(),
            questions: Vec::new(),
            is_blocking: true,
            auto_resolution_ms: None,
        }
    }

    fn exec_approval(call_id: &str, approval_id: Option<&str>) -> ExecApprovalRequestEvent {
        ExecApprovalRequestEvent {
            kind: Default::default(),
            call_id: call_id.to_string(),
            approval_id: approval_id.map(str::to_string),
            turn_id: "turn".to_string(),
            environment_id: None,
            command: vec!["true".to_string()],
            cwd: AbsolutePathBuf::current_dir().expect("current dir"),
            reason: None,
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            additional_permissions: None,
            available_decisions: None,
        }
    }

    fn command_execution(call_id: &str) -> ThreadItem {
        ThreadItem::CommandExecution {
            id: call_id.to_string(),
            command: "true".to_string(),
            cwd: AbsolutePathBuf::current_dir().expect("current dir").into(),
            process_id: None,
            plugin_id: None,
            script_path: None,
            source: CommandExecutionSource::Agent,
            status: CommandExecutionStatus::InProgress,
            command_actions: Vec::new(),
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
        }
    }

    #[test]
    fn remove_resolved_prompt_removes_matching_user_input_only() {
        let mut manager = InterruptManager::new();
        manager.push_user_input(user_input("call-a", "turn"));
        manager.push_user_input(user_input("call-b", "turn"));
        assert!(manager.has_pending_prompt());

        assert!(
            manager.remove_resolved_prompt(&ResolvedAppServerRequest::UserInput {
                call_id: "call-b".to_string(),
            })
        );

        assert_eq!(manager.queue.len(), 1);
        assert!(manager.has_pending_prompt());
        let Some(QueuedInterrupt::RequestUserInput(remaining)) = manager.queue.front() else {
            panic!("expected remaining queued user input");
        };
        assert_eq!(remaining.item_id, "call-a");
        assert!(
            manager.remove_resolved_prompt(&ResolvedAppServerRequest::UserInput {
                call_id: "call-a".to_string(),
            })
        );
        assert!(!manager.has_pending_prompt());
    }

    #[test]
    fn remove_resolved_prompt_matches_exec_approval_id() {
        let mut manager = InterruptManager::new();
        manager.push_exec_approval(exec_approval("call", Some("approval")));

        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: "thread-1".to_string(),
                id: "call".to_string(),
            })
        );
        assert_eq!(manager.queue.len(), 1);

        assert!(
            manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: "thread-1".to_string(),
                id: "approval".to_string(),
            })
        );
        assert!(manager.queue.is_empty());
    }

    #[test]
    fn remove_resolved_prompt_keeps_lifecycle_events() {
        let mut manager = InterruptManager::new();
        manager.push_item_started(command_execution("call"));
        assert!(!manager.has_pending_prompt());

        assert!(
            !manager.remove_resolved_prompt(&ResolvedAppServerRequest::ExecApproval {
                thread_id: "thread-1".to_string(),
                id: "call".to_string(),
            })
        );

        assert_eq!(manager.queue.len(), 1);
        assert!(matches!(
            manager.queue.front(),
            Some(QueuedInterrupt::ItemStarted(_))
        ));
    }
}
