//! The page can revoke preparation, but never a committed create. Shared state also
//! identifies results that can outlive their originating composer.

use crate::GitInfoProvider;
use crate::app::App;
use crate::app::AppEvent;
use crate::resolve_git_ref_with_git_info;
use crate::util::append_error_log;
use codex_cloud_tasks_client::CloudBackend;
use codex_cloud_tasks_client::CreatedTask;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, PartialEq)]
enum Phase {
    Preparing,
    Sending,
    Canceled,
}

#[derive(Clone, Debug)]
pub(crate) struct Submission(Arc<Mutex<Phase>>);

impl Submission {
    // Cancellation and request commit must compete for the same transition.
    fn transition(&self, next: Phase) -> bool {
        let mut phase = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *phase != Phase::Preparing {
            return false;
        }
        *phase = next;
        true
    }

    pub(super) fn cancel(&self) -> bool {
        self.transition(Phase::Canceled)
    }

    pub(crate) fn status(&self) -> &'static str {
        match *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Phase::Preparing => "Preparing task; Esc/Ctrl+C cancels before sending.",
            Phase::Sending => "Submitting task; cannot cancel here. Waiting for result.",
            Phase::Canceled => "Submission canceled before sending; draft kept.",
        }
    }
}

pub(crate) fn handle_key(
    app: &mut App,
    key: KeyEvent,
    backend: &Arc<dyn CloudBackend>,
    tx: &UnboundedSender<AppEvent>,
    git_info: impl GitInfoProvider + Send + Sync + 'static,
) {
    let Some(page) = app.new_task.as_mut() else {
        return;
    };
    if key.code == KeyCode::Esc
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')))
    {
        if let Some(submission) = page.submission.as_ref() {
            let canceled = submission.cancel();
            app.status = if canceled {
                submission.status().to_string()
            } else {
                "Task may already be running; waiting for the submission result.".to_string()
            };
            if canceled {
                page.submission = None;
            }
        } else {
            app.new_task = None;
            app.status = "Closed new task draft".to_string();
        }
        return;
    }
    if page.submission.is_some() {
        return;
    }
    if let codex_tui::ComposerAction::Submitted(text) = page.composer.input(key) {
        page.composer.handle_paste(text.clone());
        if let Some(env) = page.env_id.clone() {
            append_error_log(format!(
                "new-task: submit env={} size={}",
                env,
                text.chars().count()
            ));
            let submission = Submission(Arc::new(Mutex::new(Phase::Preparing)));
            page.submission = Some(submission.clone());
            app.status = "Submitting new task...".to_string();
            let tx = tx.clone();
            let backend = Arc::clone(backend);
            let best_of_n = page.best_of_n;
            tokio::spawn(async move {
                let git_ref =
                    resolve_git_ref_with_git_info(/*branch_override*/ None, &git_info).await;
                if !submission.transition(Phase::Sending) {
                    return;
                }
                if tx.send(AppEvent::NewTaskSubmissionStarted).is_err() {
                    append_error_log("new-task: UI closed before create");
                    return;
                }
                let result = backend
                    .create_task(&env, &text, &git_ref, /*qa_mode*/ false, best_of_n)
                    .await
                    .map_err(|error| error.to_string());
                if let Err(error) = tx.send(AppEvent::NewTaskSubmitted { submission, result }) {
                    append_error_log(format!("new-task: undelivered result: {:?}", error.0));
                }
            });
        } else {
            app.status = "No environment selected".to_string();
        }
    }
}

pub(crate) fn submitted(
    app: &mut App,
    submission: Submission,
    result: Result<CreatedTask, String>,
) -> bool {
    let is_current = app
        .new_task
        .as_ref()
        .and_then(|page| page.submission.as_ref())
        .is_some_and(|current| Arc::ptr_eq(&current.0, &submission.0));
    match result {
        Ok(created) => {
            append_error_log(format!("new-task: created id={}", created.id.0));
            app.status = format!("Submitted as {} — refreshing…", created.id.0);
            if is_current {
                app.new_task = None;
            }
            true
        }
        Err(msg) => {
            append_error_log(format!("new-task: submit failed: {msg}"));
            if is_current && let Some(page) = app.new_task.as_mut() {
                page.submission = None;
            }
            app.status = format!(
                "Submission not confirmed; check Cloud Tasks before retrying. {msg}. See error.log."
            );
            false
        }
    }
}

#[cfg(test)]
#[path = "submission_tests.rs"]
mod tests;
