//! Non-command tool lifecycle rendering for `ChatWidget`.
//!
//! This module handles patch, MCP, web search, image, and collaborator tool
//! events as transcript cells.

use super::*;
use codex_utils_path_uri::LegacyAppPathString;

impl ChatWidget {
    pub(super) fn on_patch_apply_begin(&mut self, changes: HashMap<PathBuf, FileChange>) {
        self.defer_or_handle(
            changes,
            InterruptManager::push_patch_apply_begin,
            Self::handle_patch_apply_begin_now,
        );
    }

    pub(crate) fn handle_patch_apply_begin_now(&mut self, changes: HashMap<PathBuf, FileChange>) {
        self.add_to_history(history_cell::new_patch_event(changes, &self.config.cwd));
    }

    pub(super) fn on_view_image_tool_call(&mut self, path: LegacyAppPathString) {
        self.defer_or_handle(
            path,
            InterruptManager::push_view_image,
            Self::handle_view_image_tool_call_now,
        );
    }

    pub(crate) fn handle_view_image_tool_call_now(&mut self, path: LegacyAppPathString) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(history_cell::new_view_image_tool_call(
            path,
            &self.config.cwd,
        ));
        self.request_redraw();
    }

    pub(super) fn on_image_generation_begin(&mut self) {
        self.defer_or_handle(
            (),
            |interrupts, ()| interrupts.push_image_generation_begin(),
            |chat, ()| chat.handle_image_generation_begin_now(),
        );
    }

    pub(crate) fn handle_image_generation_begin_now(&mut self) {
        self.flush_answer_stream_with_separator();
        if self.bottom_pane.is_task_running() {
            self.bottom_pane.ensure_status_indicator();
        }
    }

    pub(super) fn on_image_generation_end(
        &mut self,
        call_id: String,
        status: String,
        revised_prompt: Option<String>,
        saved_path: Option<AbsolutePathBuf>,
    ) {
        self.defer_or_handle(
            (call_id, status, revised_prompt, saved_path),
            |interrupts, (call_id, status, revised_prompt, saved_path)| {
                interrupts.push_image_generation_end(call_id, status, revised_prompt, saved_path);
            },
            |chat, (call_id, status, revised_prompt, saved_path)| {
                chat.handle_image_generation_end_now(call_id, status, revised_prompt, saved_path);
            },
        );
    }

    pub(crate) fn handle_image_generation_end_now(
        &mut self,
        call_id: String,
        status: String,
        revised_prompt: Option<String>,
        saved_path: Option<AbsolutePathBuf>,
    ) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(history_cell::new_image_generation_call(
            call_id,
            &status,
            revised_prompt,
            saved_path,
        ));
        self.request_redraw();
    }

    pub(super) fn on_file_change_completed(&mut self, item: ThreadItem) {
        self.defer_or_handle(
            item,
            InterruptManager::push_item_completed,
            Self::handle_file_change_completed_now,
        );
    }

    pub(super) fn on_mcp_tool_call_started(&mut self, item: ThreadItem) {
        if self.interrupts.is_empty() && self.active_mcp_group_has_incomplete_members() {
            self.handle_mcp_tool_call_started_now(item);
            return;
        }
        self.defer_or_handle(
            item,
            InterruptManager::push_item_started,
            Self::handle_mcp_tool_call_started_now,
        );
    }

    pub(super) fn on_mcp_tool_call_completed(&mut self, item: ThreadItem) {
        let owned_by_active_group = match &item {
            ThreadItem::McpToolCall { id, .. } => self.active_mcp_group_owns_call(id),
            _ => false,
        };
        if owned_by_active_group {
            self.handle_mcp_tool_call_completed_now(item);
            self.flush_interrupt_queue();
            return;
        }
        self.defer_or_handle(
            item,
            InterruptManager::push_item_completed,
            Self::handle_mcp_tool_call_completed_now,
        );
    }

    pub(super) fn on_web_search_begin(&mut self, call_id: String) {
        self.defer_or_handle(
            call_id,
            InterruptManager::push_web_search_begin,
            Self::handle_web_search_begin_now,
        );
    }

    pub(crate) fn handle_web_search_begin_now(&mut self, call_id: String) {
        self.flush_answer_stream_with_separator();
        self.flush_active_cell();
        self.transcript.active_cell = Some(Box::new(history_cell::new_active_web_search_call(
            call_id,
            String::new(),
            self.config.animations,
        )));
        self.bump_active_cell_revision();
        self.request_redraw();
    }

    pub(super) fn on_web_search_end(
        &mut self,
        call_id: String,
        query: String,
        action: codex_app_server_protocol::WebSearchAction,
    ) {
        self.defer_or_handle(
            (call_id, query, action),
            |interrupts, (call_id, query, action)| {
                interrupts.push_web_search_end(call_id, query, action);
            },
            |chat, (call_id, query, action)| {
                chat.handle_web_search_end_now(call_id, query, action);
            },
        );
    }

    pub(crate) fn handle_web_search_end_now(
        &mut self,
        call_id: String,
        query: String,
        action: codex_app_server_protocol::WebSearchAction,
    ) {
        self.flush_answer_stream_with_separator();
        let mut handled = false;
        if let Some(cell) = self
            .transcript
            .active_cell
            .as_mut()
            .and_then(|cell| cell.as_any_mut().downcast_mut::<WebSearchCell>())
            && cell.call_id() == call_id
        {
            cell.update(action.clone(), query.clone());
            cell.complete();
            self.bump_active_cell_revision();
            self.flush_active_cell();
            handled = true;
        }

        if !handled {
            self.add_to_history(history_cell::new_web_search_call(call_id, query, action));
        }
        self.transcript.had_work_activity = true;
    }

    pub(super) fn on_collab_event(&mut self, cell: PlainHistoryCell) {
        self.defer_or_handle(
            cell,
            InterruptManager::push_collab_event,
            Self::handle_collab_event_now,
        );
    }

    pub(crate) fn handle_collab_event_now(&mut self, cell: PlainHistoryCell) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(cell);
        self.request_redraw();
    }

    pub(super) fn on_collab_agent_tool_call(&mut self, item: ThreadItem) {
        let ThreadItem::CollabAgentToolCall {
            id, tool, status, ..
        } = &item
        else {
            return;
        };
        if matches!(tool, CollabAgentTool::SpawnAgent)
            && let Some(spawn_request) = multi_agents::spawn_request_summary(&item)
        {
            self.pending_collab_spawn_requests
                .insert(id.clone(), spawn_request);
        }

        let cached_spawn_request = if matches!(tool, CollabAgentTool::SpawnAgent)
            && !matches!(status, CollabAgentToolCallStatus::InProgress)
        {
            self.pending_collab_spawn_requests.remove(id)
        } else {
            None
        };

        if let Some(cell) = multi_agents::tool_call_history_cell(
            &item,
            cached_spawn_request.as_ref(),
            |thread_id| self.collab_agent_metadata(thread_id),
        ) {
            self.on_collab_event(cell);
        }
    }

    pub(super) fn on_sub_agent_activity(&mut self, item: ThreadItem) {
        if let Some(cell) = multi_agents::sub_agent_activity_history_cell(&item) {
            self.on_collab_event(cell);
        }
    }

    pub(crate) fn handle_file_change_completed_now(&mut self, item: ThreadItem) {
        let ThreadItem::FileChange { status, .. } = item else {
            return;
        };
        // If the patch was successful, just let the "Edited" block stand.
        // Otherwise, add a failure block.
        if matches!(status, codex_app_server_protocol::PatchApplyStatus::Failed) {
            self.add_to_history(history_cell::new_patch_apply_failure(String::new()));
        }
        // Mark that actual work was done (patch applied)
        self.transcript.had_work_activity = true;
    }

    pub(crate) fn handle_mcp_tool_call_started_now(&mut self, item: ThreadItem) {
        let ThreadItem::McpToolCall {
            id,
            server,
            tool,
            arguments,
            ..
        } = item
        else {
            return;
        };
        self.flush_answer_stream_with_separator();
        let invocation = McpInvocation {
            server,
            tool,
            arguments: Some(arguments),
        };
        let animations_enabled = self.config.animations;
        if let Some(group) = self.active_mcp_group_mut() {
            if group.add_started_call(id, invocation, animations_enabled) {
                self.bump_active_cell_revision();
                self.request_redraw();
            }
        } else {
            self.flush_active_cell();
            self.transcript.active_cell = Some(Box::new(history_cell::McpToolCallGroupCell::new(
                id,
                invocation,
                animations_enabled,
            )));
            self.bump_active_cell_revision();
            self.request_redraw();
        }
    }

    pub(crate) fn handle_mcp_tool_call_completed_now(&mut self, item: ThreadItem) {
        self.flush_answer_stream_with_separator();

        let ThreadItem::McpToolCall {
            id,
            server,
            tool,
            status,
            arguments,
            result,
            error,
            duration_ms,
            ..
        } = item
        else {
            return;
        };
        let invocation = McpInvocation {
            server,
            tool,
            arguments: Some(arguments),
        };
        let duration = Duration::from_millis(duration_ms.unwrap_or_default().max(0) as u64);
        let result = match (result, error) {
            (_, Some(error)) => Err(error.message),
            (Some(result), None) => {
                let result = *result;
                Ok(codex_protocol::mcp::CallToolResult {
                    content: result.content,
                    structured_content: result.structured_content,
                    is_error: Some(status == codex_app_server_protocol::McpToolCallStatus::Failed),
                    meta: None,
                })
            }
            (None, None) => Err("MCP tool call completed without a result".to_string()),
        };

        if self.active_mcp_group_owns_call(&id) {
            let completed_active_member = self
                .active_mcp_group_mut()
                .is_some_and(|group| group.complete_call(&id, duration, result));
            if !completed_active_member {
                return;
            }
            if self.active_mcp_group_has_incomplete_members() {
                self.bump_active_cell_revision();
                self.request_redraw();
            } else {
                self.flush_active_cell();
            }
        } else {
            self.flush_active_cell();
            let mut group = history_cell::McpToolCallGroupCell::new(
                id.clone(),
                invocation,
                self.config.animations,
            );
            let completed = group.complete_call(&id, duration, result);
            debug_assert!(completed, "new MCP group should contain {id}");
            self.transcript.active_cell = Some(Box::new(group));
            self.flush_active_cell();
        }
        // Mark that actual work was done (MCP tool call)
        self.transcript.had_work_activity = true;
    }

    pub(crate) fn handle_queued_item_started_now(&mut self, item: ThreadItem) {
        match item {
            item @ ThreadItem::CommandExecution { .. } => {
                self.handle_command_execution_started_now(item);
            }
            item @ ThreadItem::McpToolCall { .. } => {
                self.handle_mcp_tool_call_started_now(item);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_queued_item_completed_now(&mut self, item: ThreadItem) {
        match item {
            item @ ThreadItem::CommandExecution { .. } => {
                self.handle_command_execution_completed_now(item);
            }
            item @ ThreadItem::FileChange { .. } => self.handle_file_change_completed_now(item),
            item @ ThreadItem::McpToolCall { .. } => self.handle_mcp_tool_call_completed_now(item),
            _ => {}
        }
    }

    pub(super) fn active_mcp_group_has_incomplete_members(&self) -> bool {
        self.transcript
            .active_cell
            .as_ref()
            .and_then(|cell| {
                cell.as_any()
                    .downcast_ref::<history_cell::McpToolCallGroupCell>()
            })
            .is_some_and(history_cell::McpToolCallGroupCell::has_active_members)
    }

    pub(super) fn active_mcp_group_owns_call(&self, call_id: &str) -> bool {
        self.transcript
            .active_cell
            .as_ref()
            .and_then(|cell| {
                cell.as_any()
                    .downcast_ref::<history_cell::McpToolCallGroupCell>()
            })
            .is_some_and(|group| group.owns_call(call_id))
    }

    fn active_mcp_group_mut(&mut self) -> Option<&mut history_cell::McpToolCallGroupCell> {
        self.transcript.active_cell.as_mut().and_then(|cell| {
            cell.as_any_mut()
                .downcast_mut::<history_cell::McpToolCallGroupCell>()
        })
    }
}
