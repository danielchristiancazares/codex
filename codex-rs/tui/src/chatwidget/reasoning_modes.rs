//! Reasoning-mode selection for `ChatWidget`.

use super::ChatWidget;
use crate::app_event::AppEvent;
use codex_protocol::config_types::ReasoningMode;

impl ChatWidget {
    pub(crate) fn set_reasoning_mode(&mut self, reasoning_mode: ReasoningMode) {
        self.config.model_reasoning_mode = reasoning_mode;
    }

    pub(crate) fn current_reasoning_mode(&self) -> ReasoningMode {
        self.config.model_reasoning_mode
    }

    pub(super) fn toggle_reasoning_mode(&mut self) {
        let reasoning_mode = match self.current_reasoning_mode() {
            ReasoningMode::Standard => ReasoningMode::Pro,
            ReasoningMode::Pro => ReasoningMode::Standard,
        };
        self.set_reasoning_mode(reasoning_mode);
        self.app_event_tx
            .send(AppEvent::PersistReasoningModeSelection { reasoning_mode });
    }
}
