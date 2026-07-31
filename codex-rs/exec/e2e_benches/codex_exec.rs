use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use core_test_support::test_codex_exec::TestCodexExecBuilder;
use codex_login::CODEX_API_KEY_ENV_VAR;
use divan::Bencher;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn main() {
    divan::main();
}

struct PersistentTurnFixture {
    _runtime: tokio::runtime::Runtime,
    _server: MockServer,
    test: TestCodexExecBuilder,
    provider: String,
    response_started_rx: mpsc::Receiver<()>,
}

impl PersistentTurnFixture {
    fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
        let server = runtime.block_on(responses::start_mock_server());
        let response_body = responses::sse(vec![
            responses::ev_response_created("resp-bench"),
            responses::ev_assistant_message("msg-bench", "benchmark response"),
            responses::ev_completed("resp-bench"),
        ]);
        let (response_started_tx, response_started_rx) = mpsc::channel();
        runtime.block_on(async {
            Mock::given(method("POST"))
                .and(path("/v1/responses"))
                .respond_with(SignalingSseResponder {
                    response_body,
                    response_started_tx,
                })
                .mount(&server)
                .await;
        });
        let provider = format!(
            "model_providers.benchmark={{name=\"Benchmark\",base_url=\"{}/v1\",wire_api=\"responses\",requires_openai_auth=false,supports_websockets=false}}",
            server.uri()
        );

        Self {
            _runtime: runtime,
            _server: server,
            test: test_codex_exec(),
            provider,
            response_started_rx,
        }
    }

    fn spawn_turn(&self) -> Child {
        let codex_exec = codex_utils_cargo_bin::cargo_bin("codex-exec")
            .expect("codex exec binary should be available through Bazel runfiles");
        let child = Command::new(codex_exec)
            .current_dir(self.test.cwd_path())
            .env("CODEX_HOME", self.test.home_path())
            .env("CODEX_SQLITE_HOME", self.test.home_path())
            .env(CODEX_API_KEY_ENV_VAR, "dummy")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("--skip-git-repo-check")
            .arg("-c")
            .arg(&self.provider)
            .args(["-c", "model_provider=\"benchmark\""])
            .arg("-c")
            .arg("features.enable_request_compression=false")
            .args(["-c", "features.code_mode_host=false"])
            .arg("benchmark prompt")
            .spawn()
            .expect("codex exec benchmark command should start");
        self.response_started_rx
            .recv()
            .expect("mock response should start");
        child
    }
}

struct SignalingSseResponder {
    response_body: String,
    response_started_tx: mpsc::Sender<()>,
}

impl Respond for SignalingSseResponder {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        self.response_started_tx
            .send(())
            .expect("benchmark should wait for the response");
        responses::sse_response(self.response_body.clone())
    }
}

fn assert_turn_succeeded(child: Child) {
    let output = child
        .wait_with_output()
        .expect("codex exec benchmark command should finish");
    assert!(output.status.success(), "codex exec should succeed: {output:?}");
}

/// Measures a complete persistent `codex exec` turn against a local Responses API fixture.
#[divan::bench(sample_count = 20, sample_size = 1)]
fn persistent_turn(bencher: Bencher) {
    let fixture = PersistentTurnFixture::new();

    bencher.bench_local(move || {
        let child = fixture.spawn_turn();
        assert_turn_succeeded(child);
    });
}

/// Measures response handling, persistence, terminal completion, and process shutdown after the
/// local model begins its response. Process and session startup run outside the timed region.
#[divan::bench(sample_count = 20, sample_size = 1)]
fn response_to_exit(bencher: Bencher) {
    let fixture = PersistentTurnFixture::new();

    bencher
        .with_inputs(|| fixture.spawn_turn())
        .bench_local_values(assert_turn_succeeded);
}
