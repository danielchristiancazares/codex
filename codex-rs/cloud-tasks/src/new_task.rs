use codex_tui::ComposerInput;
use crossterm::event::KeyEvent;

pub(crate) struct NewTaskSubmission {
    pub(crate) env_id: String,
    pub(crate) prompt: String,
    pub(crate) best_of_n: usize,
}

pub(crate) enum NewTaskInput {
    None,
    MissingEnvironment,
    Submitted(NewTaskSubmission),
}

pub struct NewTaskPage {
    pub composer: ComposerInput,
    pub submitting: bool,
    pub env_id: Option<String>,
    pub best_of_n: usize,
}

impl NewTaskPage {
    pub fn new(env_id: Option<String>, best_of_n: usize) -> Self {
        let mut composer = ComposerInput::new();
        composer.set_hint_items(vec![
            ("⏎", "send"),
            ("Shift+⏎", "newline"),
            ("Ctrl+O", "env"),
            ("Ctrl+N", "attempts"),
            ("Ctrl+C", "quit"),
        ]);
        Self {
            composer,
            submitting: false,
            env_id,
            best_of_n,
        }
    }

    pub(crate) fn input(&mut self, key: KeyEvent) -> NewTaskInput {
        if self.submitting {
            return NewTaskInput::None;
        }
        let codex_tui::ComposerAction::Submitted(prompt) = self.composer.input(key) else {
            return NewTaskInput::None;
        };
        let Some(env_id) = self.env_id.clone() else {
            return NewTaskInput::MissingEnvironment;
        };

        self.submitting = true;
        NewTaskInput::Submitted(NewTaskSubmission {
            env_id,
            prompt,
            best_of_n: self.best_of_n,
        })
    }
}

impl Default for NewTaskPage {
    fn default() -> Self {
        Self::new(/*env_id*/ None, /*best_of_n*/ 1)
    }
}

#[cfg(test)]
#[path = "new_task_tests.rs"]
mod tests;
