//! Windows sandbox setup, completion handling, and persistence for the active thread.

use super::*;
#[cfg(target_os = "windows")]
use codex_config::types::WindowsSandboxModeToml;
use codex_utils_approval_presets::ApprovalPreset;

#[derive(Default)]
pub(super) struct WindowsSandboxState {
    pub(super) setup_started_at: Option<Instant>,
    // One-shot suppression of the next world-writable scan after user confirmation.
    pub(super) skip_world_writable_scan_once: bool,
    /// A startup filesystem scan can still enqueue a protected warning after the app queue drains.
    pub(super) startup_world_writable_scan_pending: bool,
}

impl App {
    pub(super) fn show_windows_sandbox_fallback(
        &mut self,
        origin_thread_id: Option<ThreadId>,
        preset: ApprovalPreset,
        profile_selection: Option<PermissionProfileSelection>,
    ) {
        if origin_thread_id.is_some() && origin_thread_id != self.current_displayed_thread_id() {
            self.windows_sandbox.setup_started_at = None;
            tracing::warn!(
                ?origin_thread_id,
                "ignoring Windows sandbox setup result for an inactive thread"
            );
            return;
        }
        self.session_telemetry.counter(
            "codex.windows_sandbox.fallback_prompt_shown",
            /*inc*/ 1,
            &[],
        );
        self.chat_widget.clear_windows_sandbox_setup_status();
        if let Some(started_at) = self.windows_sandbox.setup_started_at.take() {
            self.session_telemetry.record_duration(
                "codex.windows_sandbox.elevated_setup_duration_ms",
                started_at.elapsed(),
                &[("result", "failure")],
            );
        }
        self.chat_widget
            .open_windows_sandbox_fallback_prompt(preset, profile_selection);
    }

    pub(super) async fn begin_windows_sandbox_elevated_setup(
        &mut self,
        preset: ApprovalPreset,
        profile_selection: Option<PermissionProfileSelection>,
    ) {
        let origin_thread_id = self.current_displayed_thread_id();
        #[cfg(any(target_os = "windows", test))]
        if !self.chat_widget.windows_sandbox_mode_allowed(
            codex_config::types::WindowsSandboxModeToml::Elevated,
        ) {
            tracing::warn!(
                "refusing to set up elevated Windows sandbox mode disallowed by requirements"
            );
            self.chat_widget.add_info_message(
                "That Windows sandbox option is disallowed by requirements.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        #[cfg(target_os = "windows")]
        {
            let setup_permissions = match self
                .windows_setup_permissions(&preset, profile_selection.as_ref())
                .await
            {
                Ok(setup_permissions) => setup_permissions,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "failed to resolve permission profile for elevated Windows sandbox setup"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to prepare Windows sandbox for the selected permission profile: {err}"
                    ));
                    return;
                }
            };
            let permission_profile = setup_permissions.permission_profile;
            let workspace_roots = setup_permissions.workspace_roots;
            let command_cwd = self.config.cwd.clone();
            let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
            let codex_home = self.config.codex_home.clone();
            let tx = self.app_event_tx.clone();

            self.chat_widget.show_windows_sandbox_setup_status();
            self.windows_sandbox.setup_started_at = Some(Instant::now());
            let session_telemetry = self.session_telemetry.clone();
            tokio::task::spawn_blocking(move || {
                let result = crate::windows_sandbox::prepare_elevated_sandbox(
                    &permission_profile,
                    workspace_roots.as_slice(),
                    command_cwd.as_path(),
                    &env_map,
                    codex_home.as_path(),
                );
                let event = match result {
                    Ok(()) => {
                        session_telemetry.counter(
                            "codex.windows_sandbox.elevated_setup_success",
                            /*inc*/ 1,
                            &[],
                        );
                        AppEvent::EnableWindowsSandboxForAgentMode {
                            origin_thread_id,
                            preset: preset.clone(),
                            mode: WindowsSandboxEnableMode::Elevated,
                            profile_selection: profile_selection.clone(),
                        }
                    }
                    Err(err) => {
                        let mut code_tag: Option<String> = None;
                        let mut message_tag: Option<String> = None;
                        if let Some((code, message)) =
                            crate::windows_sandbox::elevated_setup_failure_details(&err)
                        {
                            code_tag = Some(code);
                            message_tag = Some(message);
                        }
                        let mut tags: Vec<(&str, &str)> = Vec::new();
                        if let Some(code) = code_tag.as_deref() {
                            tags.push(("code", code));
                        }
                        if let Some(message) = message_tag.as_deref() {
                            tags.push(("message", message));
                        }
                        session_telemetry.counter(
                            crate::windows_sandbox::elevated_setup_failure_metric_name(&err),
                            /*inc*/ 1,
                            &tags,
                        );
                        tracing::error!(
                            error = %err,
                            "failed to run elevated Windows sandbox setup"
                        );
                        AppEvent::OpenWindowsSandboxFallbackPrompt {
                            origin_thread_id,
                            preset,
                            profile_selection,
                        }
                    }
                };
                tx.send(event);
            });
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (origin_thread_id, preset, profile_selection);
        }
    }

    pub(super) async fn begin_windows_sandbox_legacy_setup(
        &mut self,
        preset: ApprovalPreset,
        profile_selection: Option<PermissionProfileSelection>,
    ) {
        let origin_thread_id = self.current_displayed_thread_id();
        #[cfg(any(target_os = "windows", test))]
        if !self.chat_widget.windows_sandbox_mode_allowed(
            codex_config::types::WindowsSandboxModeToml::Unelevated,
        ) {
            tracing::warn!(
                "refusing to set up unelevated Windows sandbox mode disallowed by requirements"
            );
            self.chat_widget.add_info_message(
                "That Windows sandbox option is disallowed by requirements.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        #[cfg(target_os = "windows")]
        {
            let setup_permissions = match self
                .windows_setup_permissions(&preset, profile_selection.as_ref())
                .await
            {
                Ok(setup_permissions) => setup_permissions,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "failed to resolve permission profile for legacy Windows sandbox setup"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to prepare Windows sandbox for the selected permission profile: {err}"
                    ));
                    return;
                }
            };
            let permission_profile = setup_permissions.permission_profile;
            let workspace_roots = setup_permissions.workspace_roots;
            let command_cwd = self.config.cwd.clone();
            let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
            let codex_home = self.config.codex_home.clone();
            let tx = self.app_event_tx.clone();
            let session_telemetry = self.session_telemetry.clone();

            self.chat_widget.show_windows_sandbox_setup_status();
            tokio::task::spawn_blocking(move || {
                if let Err(err) = codex_windows_sandbox::run_windows_sandbox_legacy_preflight(
                    &permission_profile,
                    workspace_roots.as_slice(),
                    codex_home.as_path(),
                    command_cwd.as_path(),
                    &env_map,
                ) {
                    session_telemetry.counter(
                        "codex.windows_sandbox.legacy_setup_preflight_failed",
                        /*inc*/ 1,
                        &[],
                    );
                    tracing::warn!(
                        error = %err,
                        "failed to preflight non-admin Windows sandbox setup"
                    );
                }
                tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                    origin_thread_id,
                    preset,
                    mode: WindowsSandboxEnableMode::Legacy,
                    profile_selection,
                });
            });
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (origin_thread_id, preset, profile_selection);
        }
    }

    pub(super) async fn enable_windows_sandbox_for_agent_mode(
        &mut self,
        app_server: &mut AppServerSession,
        origin_thread_id: Option<ThreadId>,
        preset: ApprovalPreset,
        mode: WindowsSandboxEnableMode,
        profile_selection: Option<PermissionProfileSelection>,
    ) {
        if origin_thread_id.is_some() && origin_thread_id != self.current_displayed_thread_id() {
            self.windows_sandbox.setup_started_at = None;
            tracing::warn!(
                ?origin_thread_id,
                "ignoring Windows sandbox setup result for an inactive thread"
            );
            return;
        }
        #[cfg(target_os = "windows")]
        {
            self.chat_widget.clear_windows_sandbox_setup_status();
            if let Some(started_at) = self.windows_sandbox.setup_started_at.take() {
                self.session_telemetry.record_duration(
                    "codex.windows_sandbox.elevated_setup_duration_ms",
                    started_at.elapsed(),
                    &[("result", "success")],
                );
            }
            let selected_mode = match mode {
                WindowsSandboxEnableMode::Elevated => WindowsSandboxModeToml::Elevated,
                WindowsSandboxEnableMode::Legacy => WindowsSandboxModeToml::Unelevated,
            };
            let elevated_enabled = selected_mode == WindowsSandboxModeToml::Elevated;
            if !self.chat_widget.windows_sandbox_mode_allowed(selected_mode) {
                tracing::warn!(
                    ?selected_mode,
                    "refusing to persist Windows sandbox mode disallowed by requirements"
                );
                self.chat_widget.add_info_message(
                    "That Windows sandbox option is disallowed by requirements.".to_string(),
                    /*hint*/ None,
                );
                return;
            }
            let edits = crate::config_update::build_windows_sandbox_mode_edits(elevated_enabled);
            match crate::config_update::write_config_batch(app_server.request_handle(), edits).await
            {
                Ok(response) if response.status == WriteStatus::OkOverridden => {
                    self.sync_windows_sandbox_after_overridden_write(app_server, &response)
                        .await;
                }
                Ok(_) => {
                    if elevated_enabled {
                        self.config.set_windows_sandbox_enabled(/*value*/ false);
                        self.config
                            .set_windows_elevated_sandbox_enabled(/*value*/ true);
                    } else {
                        self.config.set_windows_sandbox_enabled(/*value*/ true);
                        self.config
                            .set_windows_elevated_sandbox_enabled(/*value*/ false);
                    }
                    self.chat_widget
                        .set_windows_sandbox_mode(self.config.permissions.windows_sandbox_mode);
                    let windows_sandbox_level =
                        crate::windows_sandbox::level_from_config(&self.config);
                    if let Some((sample_paths, extra_count, failed_scan)) =
                        self.chat_widget.world_writable_warning_details()
                    {
                        self.app_event_tx.send(AppEvent::CodexOp(
                            AppCommand::override_turn_context(
                                /*cwd*/ None,
                                /*approval_policy*/ None,
                                /*approvals_reviewer*/ None,
                                /*permission_profile*/ None,
                                /*active_permission_profile*/ None,
                                Some(windows_sandbox_level),
                                /*model*/ None,
                                /*effort*/ None,
                                /*summary*/ None,
                                /*service_tier*/ None,
                                /*collaboration_mode*/ None,
                                /*personality*/ None,
                            ),
                        ));
                        self.app_event_tx
                            .send(AppEvent::OpenWorldWritableWarningConfirmation {
                                preset: Some(preset.clone()),
                                profile_selection: profile_selection.clone(),
                                sample_paths,
                                extra_count,
                                failed_scan,
                            });
                    } else if let Some(selection) = profile_selection {
                        self.app_event_tx.send(AppEvent::CodexOp(
                            AppCommand::override_turn_context(
                                /*cwd*/ None,
                                /*approval_policy*/ None,
                                /*approvals_reviewer*/ None,
                                /*permission_profile*/ None,
                                /*active_permission_profile*/ None,
                                Some(windows_sandbox_level),
                                /*model*/ None,
                                /*effort*/ None,
                                /*summary*/ None,
                                /*service_tier*/ None,
                                /*collaboration_mode*/ None,
                                /*personality*/ None,
                            ),
                        ));
                        if self.apply_permission_profile_selection(selection).await {
                            self.chat_widget.submit_initial_user_message_if_pending();
                        }
                        self.chat_widget.add_plain_history_lines(vec![
                            Line::from(vec!["• ".dim(), "Sandbox ready".into()]),
                            Line::from(vec![
                                "  ".into(),
                                "Codex can now safely edit files and execute commands in your computer"
                                    .dark_gray(),
                            ]),
                        ]);
                    } else {
                        self.app_event_tx.send(AppEvent::CodexOp(
                            AppCommand::override_turn_context(
                                /*cwd*/ None,
                                Some(AskForApproval::from(preset.approval)),
                                Some(self.config.approvals_reviewer),
                                Some(preset.permission_profile.clone()),
                                Some(preset.active_permission_profile.clone()),
                                Some(windows_sandbox_level),
                                /*model*/ None,
                                /*effort*/ None,
                                /*summary*/ None,
                                /*service_tier*/ None,
                                /*collaboration_mode*/ None,
                                /*personality*/ None,
                            ),
                        ));
                        self.app_event_tx.send(AppEvent::UpdateAskForApprovalPolicy(
                            AskForApproval::from(preset.approval),
                        ));
                        self.app_event_tx.send(AppEvent::UpdateActivePermissionProfile(
                            preset.active_permission_profile.clone(),
                        ));
                        self.chat_widget.add_plain_history_lines(vec![
                            Line::from(vec!["• ".dim(), "Sandbox ready".into()]),
                            Line::from(vec![
                                "  ".into(),
                                "Codex can now safely edit files and execute commands in your computer"
                                    .dark_gray(),
                            ]),
                        ]);
                    }
                }
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "failed to enable Windows sandbox feature"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to enable the Windows sandbox feature: {err}"
                    ));
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (app_server, preset, mode, profile_selection);
        }
    }
}
