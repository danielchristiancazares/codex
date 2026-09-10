use super::*;
use crate::new_task::NewTaskPage;
use codex_cloud_tasks_client::*;
use codex_cloud_tasks_mock_client::MockClient;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::oneshot;

#[derive(Default)]
struct Gate {
    entered: Notify,
    release: Notify,
}

struct GatedGit {
    gate: Arc<Gate>,
    done: Option<oneshot::Sender<()>>,
}

impl GitInfoProvider for GatedGit {
    async fn current_branch_name(&self, _path: &Path) -> Option<String> {
        self.gate.entered.notify_one();
        self.gate.release.notified().await;
        Some("feature/test".to_string())
    }

    async fn default_branch_name(&self, _path: &Path) -> Option<String> {
        panic!("current branch should be used")
    }
}

impl Drop for GatedGit {
    fn drop(&mut self) {
        if let Some(done) = self.done.take() {
            let _ = done.send(());
        }
    }
}

type CreateRequest = (String, String, String, bool, usize);

#[derive(Default)]
struct RecordingBackend {
    requests: Mutex<Vec<CreateRequest>>,
    create: Gate,
    fail: bool,
}

impl CloudBackend for RecordingBackend {
    fn list_tasks<'a>(
        &'a self,
        env: Option<&'a str>,
        limit: Option<i64>,
        cursor: Option<&'a str>,
    ) -> CloudBackendFuture<'a, TaskListPage> {
        MockClient.list_tasks(env, limit, cursor)
    }
    fn get_task_summary(&self, id: TaskId) -> CloudBackendFuture<'_, TaskSummary> {
        MockClient.get_task_summary(id)
    }
    fn get_task_diff(&self, id: TaskId) -> CloudBackendFuture<'_, Option<String>> {
        MockClient.get_task_diff(id)
    }
    fn get_task_messages(&self, id: TaskId) -> CloudBackendFuture<'_, Vec<String>> {
        MockClient.get_task_messages(id)
    }
    fn get_task_text(&self, id: TaskId) -> CloudBackendFuture<'_, TaskText> {
        MockClient.get_task_text(id)
    }
    fn list_sibling_attempts(
        &self,
        id: TaskId,
        turn_id: String,
    ) -> CloudBackendFuture<'_, Vec<TurnAttempt>> {
        MockClient.list_sibling_attempts(id, turn_id)
    }
    fn apply_task_preflight(
        &self,
        id: TaskId,
        diff: Option<String>,
    ) -> CloudBackendFuture<'_, ApplyOutcome> {
        MockClient.apply_task_preflight(id, diff)
    }
    fn apply_task(&self, id: TaskId, diff: Option<String>) -> CloudBackendFuture<'_, ApplyOutcome> {
        MockClient.apply_task(id, diff)
    }
    fn create_task<'a>(
        &'a self,
        env: &'a str,
        prompt: &'a str,
        git_ref: &'a str,
        qa_mode: bool,
        best_of_n: usize,
    ) -> CloudBackendFuture<'a, CreatedTask> {
        Box::pin(async move {
            self.requests.lock().unwrap().push((
                env.to_string(),
                prompt.to_string(),
                git_ref.to_string(),
                qa_mode,
                best_of_n,
            ));
            self.create.entered.notify_one();
            self.create.release.notified().await;
            if self.fail {
                Err(CloudTaskError::Msg(
                    "connection lost after sending".to_string(),
                ))
            } else {
                Ok(CreatedTask {
                    id: TaskId("task-created".to_string()),
                })
            }
        })
    }
}

fn draft(prompt: &str) -> NewTaskPage {
    let mut page = NewTaskPage::new(Some("env-test".to_string()), /*best_of_n*/ 4);
    page.composer.handle_paste(prompt.to_string());
    page
}

fn start(
    app: &mut App,
    backend: &Arc<RecordingBackend>,
    tx: &UnboundedSender<AppEvent>,
) -> (Arc<Gate>, oneshot::Receiver<()>) {
    let gate = Arc::new(Gate::default());
    let (done, receiver) = oneshot::channel();
    let backend: Arc<dyn CloudBackend> = backend.clone();
    handle_key(
        app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &backend,
        tx,
        GatedGit {
            gate: gate.clone(),
            done: Some(done),
        },
    );
    (gate, receiver)
}

fn cancel(app: &mut App, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    let backend: Arc<dyn CloudBackend> = Arc::new(MockClient);
    handle_key(app, key, &backend, tx, crate::RealGitInfo);
}

fn cancel_keys() -> [KeyEvent; 2] {
    [
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    ]
}

async fn complete(app: &mut App, rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>) {
    assert!(matches!(
        rx.recv().await.unwrap(),
        AppEvent::NewTaskSubmissionStarted
    ));
    let AppEvent::NewTaskSubmitted { submission, result } = rx.recv().await.unwrap() else {
        panic!("expected submission result")
    };
    submitted(app, submission, result);
}

fn render(app: &mut App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
        /*width*/ 80, /*height*/ 12,
    ))
    .unwrap();
    terminal.draw(|frame| crate::ui::draw(frame, app)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(80)
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn cancellation_during_preparation_prevents_create() {
    for key in cancel_keys() {
        let mut app = App::new();
        app.new_task = Some(draft("first prompt"));
        let backend = Arc::new(RecordingBackend::default());
        let (tx, mut rx) = unbounded_channel();
        let (git, done) = start(&mut app, &backend, &tx);
        git.entered.notified().await;
        cancel(&mut app, key, &tx);
        backend.create.release.notify_one();
        git.release.notify_one();
        done.await.unwrap();
        assert_eq!(*backend.requests.lock().unwrap(), Vec::new());
        assert!(rx.try_recv().is_err());
        insta::allow_duplicates! {
            insta::assert_snapshot!(app.status, @"Submission canceled before sending; draft kept.");
        }

        let (git, done) = start(&mut app, &backend, &tx);
        git.release.notify_one();
        done.await.unwrap();
        assert_eq!(
            *backend.requests.lock().unwrap(),
            vec![(
                "env-test".to_string(),
                "first prompt".to_string(),
                "feature/test".to_string(),
                false,
                4,
            )]
        );
        complete(&mut app, &mut rx).await;
        assert!(app.new_task.is_none());
        assert!(app.status.contains("task-created"));
    }
}

#[tokio::test]
async fn in_flight_create_cannot_be_represented_as_cancelled() {
    for key in cancel_keys() {
        let mut app = App::new();
        app.new_task = Some(draft("first prompt"));
        let backend = Arc::new(RecordingBackend::default());
        let (tx, mut rx) = unbounded_channel();
        let (git, done) = start(&mut app, &backend, &tx);
        git.release.notify_one();
        backend.create.entered.notified().await;
        cancel(&mut app, key, &tx);
        assert!(
            app.new_task
                .as_ref()
                .is_some_and(|page| page.submission.is_some())
        );
        assert!(!app.status.contains("Canceled"));
        insta::allow_duplicates! {
            insta::assert_snapshot!(render(&mut app), @r"
            ┌New Task  • env-test  • 4 attempts────────────────────────────────────────────┐
            │                                                                              │
            │                                                                              │
            │                                                                              │
            │                                                                              │
            │                                                                              │
            │╭────────────────────────────────────────────────────────────────────────────╮│
            │┃ › first prompt                                                             ││
            │╰────────────────────────────────────────────────────────────────────────────╯│
            └──────────────────────────────────────────────────────────────────────────────┘
            Submitting task; cannot cancel here. Waiting for result.
            Task may already be running; waiting for the submission result.
            ");
        }
        let (duplicate_git, duplicate_done) = start(&mut app, &backend, &tx);
        duplicate_git.release.notify_one();
        backend.create.release.notify_one();
        done.await.unwrap();
        backend.create.release.notify_one();
        duplicate_done.await.unwrap();
        assert_eq!(backend.requests.lock().unwrap().len(), 1);
        cancel(&mut app, key, &tx);
        assert!(app.new_task.is_some());
        complete(&mut app, &mut rx).await;
        assert!(app.new_task.is_none());
        assert!(app.status.contains("task-created"));
    }
}

#[tokio::test]
async fn late_completion_does_not_clear_a_later_composer() {
    for fail in [false, true] {
        let mut app = App::new();
        app.new_task = Some(draft("first prompt"));
        let backend = Arc::new(RecordingBackend {
            fail,
            ..Default::default()
        });
        let (tx, mut rx) = unbounded_channel();
        let (git, done) = start(&mut app, &backend, &tx);
        git.release.notify_one();
        backend.create.release.notify_one();
        done.await.unwrap();
        app.new_task = Some(draft("later prompt"));
        let (later_git, later_done) = start(&mut app, &backend, &tx);
        later_git.entered.notified().await;
        complete(&mut app, &mut rx).await;
        assert!(
            app.new_task
                .as_ref()
                .is_some_and(|page| page.submission.is_some())
        );
        assert!(app.status.contains(if fail {
            "connection lost"
        } else {
            "task-created"
        }));
        cancel(&mut app, cancel_keys()[0], &tx);
        later_git.release.notify_one();
        later_done.await.unwrap();
        assert_eq!(backend.requests.lock().unwrap().len(), 1);
        let codex_tui::ComposerAction::Submitted(text) = app
            .new_task
            .as_mut()
            .unwrap()
            .composer
            .input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        else {
            panic!("later draft should be preserved")
        };
        assert_eq!(text, "later prompt");
    }
}

#[tokio::test]
async fn missing_environment_and_failed_create_preserve_the_draft() {
    let mut app = App::new();
    let mut page = draft("keep this prompt");
    page.env_id = None;
    app.new_task = Some(page);
    let backend = Arc::new(RecordingBackend {
        fail: true,
        ..Default::default()
    });
    let (tx, mut rx) = unbounded_channel();
    let (_, done) = start(&mut app, &backend, &tx);
    done.await.unwrap();
    assert_eq!(*backend.requests.lock().unwrap(), Vec::new());
    app.new_task.as_mut().unwrap().env_id = Some("env-test".to_string());
    let (git, done) = start(&mut app, &backend, &tx);
    git.release.notify_one();
    backend.create.release.notify_one();
    done.await.unwrap();
    complete(&mut app, &mut rx).await;
    insta::assert_snapshot!(app.status, @"Submission not confirmed; check Cloud Tasks before retrying. connection lost after sending. See error.log.");
    let page = app.new_task.as_mut().unwrap();
    assert!(page.submission.is_none());
    let codex_tui::ComposerAction::Submitted(text) = page
        .composer
        .input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    else {
        panic!("failed submission should retain its draft")
    };
    assert_eq!(text, "keep this prompt");
}

#[tokio::test]
async fn dropping_the_composer_revokes_preparation() {
    let mut app = App::new();
    app.new_task = Some(draft("first prompt"));
    let backend = Arc::new(RecordingBackend::default());
    let (tx, _rx) = unbounded_channel();
    let (git, done) = start(&mut app, &backend, &tx);
    git.entered.notified().await;
    app.new_task = Some(draft("replacement draft"));
    backend.create.release.notify_one();
    git.release.notify_one();
    done.await.unwrap();
    assert_eq!(*backend.requests.lock().unwrap(), Vec::new());
}
