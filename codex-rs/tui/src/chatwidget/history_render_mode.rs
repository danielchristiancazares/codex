//! Render-mode changes respect ownership of streamed terminal history.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum PendingHistoryRenderMode {
    #[default]
    None,
    ApplyAfterCurrentStream(HistoryRenderMode),
}

impl ChatWidget {
    pub(crate) fn raw_output_mode(&self) -> bool {
        self.raw_output_mode
    }

    pub(crate) fn requested_raw_output_mode(&self) -> bool {
        match self.pending_history_render_mode {
            PendingHistoryRenderMode::None => self.raw_output_mode,
            PendingHistoryRenderMode::ApplyAfterCurrentStream(HistoryRenderMode::Rich) => false,
            PendingHistoryRenderMode::ApplyAfterCurrentStream(HistoryRenderMode::Raw) => true,
        }
    }

    pub(crate) fn history_render_mode(&self) -> HistoryRenderMode {
        if self.raw_output_mode {
            HistoryRenderMode::Raw
        } else {
            HistoryRenderMode::Rich
        }
    }

    pub(crate) fn history_stream_active(&self) -> bool {
        self.stream_controller.is_some()
            || self.plan_stream_controller.is_some()
            || self.pending_stream_consolidations > 0
    }

    pub(crate) fn raw_output_mode_request_will_delay(&self, enabled: bool) -> bool {
        self.transcript_replay_policy == TranscriptReplayPolicy::InlinePreserveScrollback
            && self.history_stream_active()
            && enabled != self.raw_output_mode
    }

    #[cfg(test)]
    pub(crate) fn set_transcript_replay_policy_for_tests(
        &mut self,
        policy: TranscriptReplayPolicy,
    ) {
        self.transcript_replay_policy = policy;
    }

    pub(crate) fn defer_raw_output_mode_until_stream_boundary(&mut self, enabled: bool) {
        let requested_mode = if enabled {
            HistoryRenderMode::Raw
        } else {
            HistoryRenderMode::Rich
        };
        self.pending_history_render_mode = if requested_mode == self.history_render_mode() {
            PendingHistoryRenderMode::None
        } else {
            PendingHistoryRenderMode::ApplyAfterCurrentStream(requested_mode)
        };
    }

    pub(crate) fn take_pending_raw_output_mode_after_stream(&mut self) -> Option<bool> {
        if self.history_stream_active() {
            return None;
        }
        match std::mem::take(&mut self.pending_history_render_mode) {
            PendingHistoryRenderMode::None => None,
            PendingHistoryRenderMode::ApplyAfterCurrentStream(HistoryRenderMode::Rich) => {
                Some(false)
            }
            PendingHistoryRenderMode::ApplyAfterCurrentStream(HistoryRenderMode::Raw) => Some(true),
        }
    }

    pub(crate) fn set_raw_output_mode(&mut self, enabled: bool) {
        self.pending_history_render_mode = PendingHistoryRenderMode::None;
        let streams_were_idle = self.stream_controllers_idle();
        self.raw_output_mode = enabled;
        self.local_settings.tui.raw_output_mode = enabled;
        let render_mode = self.history_render_mode();
        if let Some(controller) = self.stream_controller.as_mut() {
            controller.set_render_mode(render_mode);
        }
        if let Some(controller) = self.plan_stream_controller.as_mut() {
            controller.set_render_mode(render_mode);
        }
        let stream_tail_changed = self.sync_active_stream_tail();
        if streams_were_idle && !self.stream_controllers_idle() {
            self.app_event_tx.send(AppEvent::StartCommitAnimation);
            self.run_catch_up_commit_tick();
        }
        self.refresh_status_surfaces();
        if stream_tail_changed {
            self.request_redraw();
        }
    }

    pub(crate) fn raw_output_mode_notice(enabled: bool) -> &'static str {
        if enabled {
            "Raw output mode on: transcript text is shown for clean terminal selection."
        } else {
            "Raw output mode off: rich transcript rendering restored."
        }
    }

    #[cfg(test)]
    pub(crate) fn set_raw_output_mode_and_notify(&mut self, enabled: bool) {
        self.set_raw_output_mode(enabled);
        self.add_info_message(
            Self::raw_output_mode_notice(enabled).to_string(),
            /*hint*/ None,
        );
    }

    pub(crate) fn acknowledge_raw_output_mode_request(&mut self, enabled: bool) {
        let message = if self.raw_output_mode_request_will_delay(enabled) {
            "Raw output will apply after the current response."
        } else {
            Self::raw_output_mode_notice(enabled)
        };
        self.add_info_message(message.to_string(), /*hint*/ None);
    }
}
