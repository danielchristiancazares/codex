#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::collections::HashSet;
use std::process::Stdio;
use std::time::Duration;

use codex_code_mode_protocol::host;
use divan::Bencher;
use serde_json::json;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);
const BATCH_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 20);
const CHILD_EXIT_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 7);
const SMALL_BATCH_SIZE: usize = 8;
const LARGE_BATCH_SIZE: usize = 4;
const SMALL_PAYLOAD_BYTES: usize = 64;
const LARGE_PAYLOAD_BYTES: usize = 8 * 1024;
const EXECUTION_YIELD_TIME_MS: u64 = 5_000;
const MAX_OUTPUT_TOKENS: i32 = 32_000;
const EXPECTED_DELEGATES_PER_CELL: usize = 1;

fn main() {
    divan::main();
}

struct HostConnection {
    child: Child,
    reader: host::FramedReader<ChildStdout>,
    writer: Option<host::FramedWriter<ChildStdin>>,
    session_id: host::SessionId,
    next_request_id: i64,
}

impl HostConnection {
    async fn start() -> Self {
        let host_program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")
            .expect("code-mode host binary should be available through Bazel runfiles");
        let mut command = Command::new(host_program);
        command
            .arg("--listen")
            .arg("stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .expect("stdio code-mode host should start");
        let stdin = child.stdin.take().expect("host stdin should be piped");
        let stdout = child.stdout.take().expect("host stdout should be piped");
        let mut connection = Self {
            child,
            reader: host::FramedReader::new(stdout),
            writer: Some(host::FramedWriter::new(stdin)),
            session_id: host::SessionId::new("throughput-benchmark")
                .expect("benchmark session ID should be valid"),
            next_request_id: 1,
        };

        let hello = host::ClientHello::new(
            host::SupportedProtocolVersions::try_new([host::ProtocolVersion::V1])
                .expect("protocol version set should be valid"),
            host::CapabilitySet::empty(),
            host::CapabilitySet::empty(),
        )
        .expect("client hello should be valid");
        connection.send(&host::ClientToHost::ClientHello(hello)).await;
        assert_eq!(
            connection.read().await,
            host::HostToClient::HostHello(host::HostHello::new(
                host::ProtocolVersion::V1,
                host::CapabilitySet::empty(),
            ))
        );

        let (request_id, _) = connection.allocate_request_id();
        connection
            .send(&host::ClientToHost::Request {
                id: request_id,
                request: host::HostRequest::OpenSession {
                    session_id: connection.session_id.clone(),
                },
            })
            .await;
        assert_eq!(
            connection.read().await,
            host::HostToClient::Response {
                id: request_id,
                result: host::WireResult::Ok {
                    value: host::HostResponse::SessionReady {
                        session_id: connection.session_id.clone(),
                    },
                },
            }
        );

        connection
    }

    async fn run_batch(&mut self, batch_size: usize, payload: &str) {
        let mut states = HashMap::with_capacity(batch_size);
        let payload_json =
            serde_json::to_string(payload).expect("benchmark payload should serialize");
        let echo_tool = host::WireToolDefinition {
            name: "echo".to_string(),
            tool_name: host::WireToolName {
                name: "echo".to_string(),
                namespace: None,
            },
            description: String::new(),
            kind: host::WireToolKind::Function,
            input_schema: None,
            output_schema: None,
        };

        for _ in 0..batch_size {
            let (request_id, sequence) = self.allocate_request_id();
            assert!(states.insert(request_id, (None, None)).is_none());
            let source = format!(
                "const result = await tools.echo({{ sequence: {sequence}, payload: {payload_json} }}); text(result.payload);"
            );
            self.send(&host::ClientToHost::Request {
                id: request_id,
                request: host::HostRequest::Execute {
                    session_id: self.session_id.clone(),
                    request: host::WireExecuteRequest {
                        tool_call_id: format!("throughput-bench-{sequence}"),
                        enabled_tools: vec![echo_tool.clone()],
                        source,
                        yield_time_ms: Some(EXECUTION_YIELD_TIME_MS),
                        max_output_tokens: Some(MAX_OUTPUT_TOKENS),
                    },
                },
            })
            .await;
        }

        let mut closed_cells = HashSet::with_capacity(batch_size);
        let mut delegate_counts = HashMap::<host::WireCellId, usize>::with_capacity(batch_size);
        loop {
            match self.read().await {
                host::HostToClient::Response { id, result } => {
                    let state = states
                        .get_mut(&id)
                        .expect("response should belong to the current batch");
                    let host::HostResponse::ExecutionStarted { cell_id } =
                        result.into_result().expect("execution should start")
                    else {
                        panic!("execute request returned an unexpected response");
                    };
                    assert!(state.0.replace(cell_id).is_none());
                }
                host::HostToClient::InitialResponse { id, result } => {
                    let state = states
                        .get_mut(&id)
                        .expect("initial response should belong to the current batch");
                    let host::WireRuntimeResponse::Result {
                        cell_id,
                        content_items,
                        error_text,
                    } = result.into_result().expect("execution should complete")
                    else {
                        panic!("benchmark execution unexpectedly yielded or terminated");
                    };
                    assert_eq!(error_text, None);
                    assert_eq!(
                        content_items,
                        vec![host::WireContentItem::InputText {
                            text: payload.to_string(),
                        }]
                    );
                    assert!(state.1.replace(cell_id).is_none());
                }
                host::HostToClient::DelegateRequest {
                    id,
                    session_id,
                    request,
                } => {
                    assert_eq!(session_id, self.session_id);
                    let host::DelegateRequest::InvokeTool { invocation } = request else {
                        panic!("benchmark execution sent an unexpected notification");
                    };
                    assert_eq!(invocation.tool_name.name, "echo");
                    let cell_id = invocation.cell_id;
                    let result = invocation
                        .input
                        .expect("echo invocation should contain its payload");
                    let sequence = result
                        .get("sequence")
                        .and_then(serde_json::Value::as_i64)
                        .expect("echo invocation should contain its sequence");
                    let request_id = host::RequestId::new(sequence);
                    assert!(states.contains_key(&request_id));
                    assert_eq!(
                        result,
                        json!({
                            "sequence": sequence,
                            "payload": payload,
                        })
                    );
                    *delegate_counts.entry(cell_id).or_default() += 1;
                    self.send(&host::ClientToHost::DelegateResponse {
                        id,
                        result: host::WireResult::Ok {
                            value: host::DelegateResponse::ToolResult { result },
                        },
                    })
                    .await;
                }
                host::HostToClient::CellClosed {
                    session_id,
                    cell_id,
                } => {
                    assert_eq!(session_id, self.session_id);
                    assert!(closed_cells.insert(cell_id));
                }
                host::HostToClient::HostHello(_)
                | host::HostToClient::HandshakeRejected { .. }
                | host::HostToClient::CancelDelegateRequest { .. } => {
                    panic!("unexpected host frame during execute batch");
                }
            }

            if states
                .values()
                .all(|(response, initial)| response.is_some() && initial.is_some())
                && closed_cells.len() == batch_size
                && delegate_counts.values().sum::<usize>() == batch_size
            {
                break;
            }
        }

        for (response_cell, initial_cell) in states.values() {
            let response_cell = response_cell
                .as_ref()
                .expect("execution-started response should include a cell");
            assert_eq!(initial_cell.as_ref(), Some(response_cell));
            assert!(closed_cells.contains(response_cell));
            assert_eq!(
                delegate_counts.get(response_cell).copied(),
                Some(EXPECTED_DELEGATES_PER_CELL)
            );
        }
    }

    async fn send(&mut self, message: &host::ClientToHost) {
        self.writer
            .as_mut()
            .expect("host stdin should remain open")
            .write(message)
            .await
            .expect("host frame should be written");
    }

    async fn read(&mut self) -> host::HostToClient {
        self.reader
            .read()
            .await
            .expect("host frame should be readable")
            .expect("host should not close stdout")
    }

    fn allocate_request_id(&mut self) -> (host::RequestId, i64) {
        let request_id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(/*rhs*/ 1)
            .expect("benchmark request IDs should not overflow");
        (host::RequestId::new(request_id), request_id)
    }
}

struct BenchmarkFixture {
    runtime: tokio::runtime::Runtime,
    connection: HostConnection,
    batch_size: usize,
    payload: String,
}

impl BenchmarkFixture {
    fn new(batch_size: usize, payload_bytes: usize) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("benchmark runtime should start");
        let payload = "x".repeat(payload_bytes);
        let mut connection = runtime
            .block_on(async {
                tokio::time::timeout(STARTUP_TIMEOUT, HostConnection::start()).await
            })
            .expect("host startup and negotiation should be bounded");
        runtime
            .block_on(async {
                tokio::time::timeout(
                    BATCH_TIMEOUT,
                    connection.run_batch(/*batch_size*/ 1, &payload),
                )
                .await
            })
            .expect("warm-up execution should be bounded");
        Self {
            runtime,
            connection,
            batch_size,
            payload,
        }
    }

    fn run_batch(&mut self) {
        self.runtime
            .block_on(async {
                tokio::time::timeout(
                    BATCH_TIMEOUT,
                    self.connection.run_batch(self.batch_size, &self.payload),
                )
                .await
            })
            .expect("timed execute batch should be bounded");
    }
}

impl Drop for BenchmarkFixture {
    fn drop(&mut self) {
        self.connection.writer.take();
        let exited = self.runtime.block_on(async {
            tokio::time::timeout(CHILD_EXIT_TIMEOUT, self.connection.child.wait()).await
        });
        if !matches!(exited, Ok(Ok(_))) {
            let _ = self.connection.child.start_kill();
            let _ = self.runtime.block_on(async {
                tokio::time::timeout(CHILD_EXIT_TIMEOUT, self.connection.child.wait()).await
            });
        }
    }
}

fn bench_delegate_batch(bencher: Bencher, batch_size: usize, payload_bytes: usize) {
    let mut fixture = BenchmarkFixture::new(batch_size, payload_bytes);
    bencher.bench_local(move || fixture.run_batch());
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn small_delegate_batch(bencher: Bencher) {
    bench_delegate_batch(bencher, SMALL_BATCH_SIZE, SMALL_PAYLOAD_BYTES);
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn large_delegate_batch(bencher: Bencher) {
    bench_delegate_batch(bencher, LARGE_BATCH_SIZE, LARGE_PAYLOAD_BYTES);
}
