use codex_tui::ComposerInput;

pub(crate) mod submission;

pub struct NewTaskPage {
    pub composer: ComposerInput,
    pub submission: Option<submission::Submission>,
    pub env_id: Option<String>,
    pub best_of_n: usize,
}

impl NewTaskPage {
    pub fn new(env_id: Option<String>, best_of_n: usize) -> Self {
        let mut page = Self {
            composer: ComposerInput::new(),
            submission: None,
            env_id,
            best_of_n,
        };
        page.update_hints();
        page
    }

    pub fn update_hints(&mut self) {
        self.composer.set_hint_items(if self.submission.is_some() {
            Vec::new()
        } else {
            vec![
                ("⏎", "send"),
                ("Shift+⏎", "newline"),
                ("Ctrl+O", "env"),
                ("Ctrl+N", "attempts"),
                ("Ctrl+C", "close"),
            ]
        });
    }
}

impl Drop for NewTaskPage {
    fn drop(&mut self) {
        if let Some(submission) = &self.submission {
            submission.cancel();
        }
    }
}

impl Default for NewTaskPage {
    fn default() -> Self {
        Self::new(/*env_id*/ None, /*best_of_n*/ 1)
    }
}
