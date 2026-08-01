use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use codex_code_mode_protocol::host::*;
use divan::Bencher;
use futures::future::try_join_all;
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpStream,
    process::{Child, Command},
    runtime::Runtime,
    time::timeout,
};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;

const IO_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);
const BATCH_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 20);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 2);
const SMALL_BATCH_SIZE: usize = 8;
const LARGE_BATCH_SIZE: usize = 4;
const SMALL_PAYLOAD_BYTES: usize = 64;
const LARGE_PAYLOAD_BYTES: usize = 8 * 1024;
const DELEGATE_SERIALIZATION_BATCH_SIZE: usize = 12;
const DELEGATE_SERIALIZATION_PAYLOAD_BYTES: usize = 64 * 1024;
const TRIVIAL_CELL_BATCH_SIZE: usize = 32;
const MULTI_CLIENT_COUNT: usize = 8;
const MULTI_CLIENT_BATCH_SIZE: usize = 8;
const EXECUTION_YIELD_TIME_MS: u64 = 5_000;
const MAX_OUTPUT_TOKENS: i32 = 32_000;

fn main() {
    divan::main();
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn small_payload_batch(bencher: Bencher) {
    bench_websocket(bencher, SMALL_BATCH_SIZE, SMALL_PAYLOAD_BYTES);
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn large_payload_batch(bencher: Bencher) {
    bench_websocket(bencher, LARGE_BATCH_SIZE, LARGE_PAYLOAD_BYTES);
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn concurrent_large_delegate_serialization(bencher: Bencher) {
    let mut fixture =
        BenchmarkFixture::new(DELEGATE_SERIALIZATION_PAYLOAD_BYTES, BatchWorkload::SmallOutput)
            .expect("delegate serialization benchmark fixture should initialize and warm");
    bencher.bench_local(move || {
        fixture
            .run_batch(DELEGATE_SERIALIZATION_BATCH_SIZE)
            .expect("delegate serialization batch should complete");
    });
}

#[divan::bench(sample_count = 30, sample_size = 1)]
fn concurrent_trivial_cells(bencher: Bencher) {
    let mut fixture = BenchmarkFixture::new(/*payload_bytes*/ 0, BatchWorkload::TrivialCell)
        .expect("trivial-cell benchmark fixture should initialize and warm");
    bencher.bench_local(move || {
        fixture
            .run_batch(TRIVIAL_CELL_BATCH_SIZE)
            .expect("trivial-cell batch should complete");
    });
}

#[divan::bench(sample_count = 50, sample_size = 1)]
fn sequential_payload_round_trip(bencher: Bencher) {
    bench_websocket(bencher, /*batch_size*/ 1, SMALL_PAYLOAD_BYTES);
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn multi_client_small_payload_round_trips(bencher: Bencher) {
    let mut fixture =
        MultiClientBenchmarkFixture::new(MULTI_CLIENT_COUNT, SMALL_PAYLOAD_BYTES)
            .expect("multi-client WebSocket benchmark fixture should initialize and warm");
    bencher.bench_local(move || {
        fixture
            .run_batch(MULTI_CLIENT_BATCH_SIZE)
            .expect("multi-client WebSocket batch should complete");
    });
}

fn bench_websocket(bencher: Bencher, batch_size: usize, payload_bytes: usize) {
    let mut fixture = BenchmarkFixture::new(payload_bytes, BatchWorkload::EchoPayload)
        .expect("code-mode WebSocket benchmark fixture should initialize and warm");
    bencher.bench_local(move || {
        fixture.run_batch(batch_size).expect("WebSocket batch should complete");
    });
}

struct BenchmarkFixture {
    runtime: Runtime,
    state: HostState,
    workload: BatchWorkload,
}

impl BenchmarkFixture {
    fn new(payload_bytes: usize, workload: BatchWorkload) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to start benchmark runtime")?;
        let mut state = runtime.block_on(HostState::start(payload_bytes))?;
        runtime.block_on(state.run_batch(/*batch_size*/ 1, workload))?;
        Ok(Self {
            runtime,
            state,
            workload,
        })
    }

    fn run_batch(&mut self, batch_size: usize) -> Result<()> {
        self.runtime
            .block_on(self.state.run_batch(batch_size, self.workload))
    }
}

impl Drop for BenchmarkFixture {
    fn drop(&mut self) {
        let state = &mut self.state;
        self.runtime.block_on(async {
            let close = state.client.client.websocket.close(/*msg*/ None);
            let _ = timeout(SHUTDOWN_TIMEOUT, close).await;
            if !matches!(state.child.try_wait(), Ok(Some(_))) {
                let _ = state.child.start_kill();
                let _ = timeout(SHUTDOWN_TIMEOUT, state.child.wait()).await;
            }
        });
    }
}

struct MultiClientBenchmarkFixture {
    runtime: Runtime,
    state: MultiClientHostState,
}

impl MultiClientBenchmarkFixture {
    fn new(client_count: usize, payload_bytes: usize) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to start multi-client benchmark runtime")?;
        let mut state =
            runtime.block_on(MultiClientHostState::start(client_count, payload_bytes))?;
        runtime.block_on(state.run_batch(/*batch_size*/ 1))?;
        Ok(Self { runtime, state })
    }

    fn run_batch(&mut self, batch_size: usize) -> Result<()> {
        self.runtime.block_on(self.state.run_batch(batch_size))
    }
}

impl Drop for MultiClientBenchmarkFixture {
    fn drop(&mut self) {
        let state = &mut self.state;
        self.runtime.block_on(async {
            let closes = state
                .clients
                .iter_mut()
                .map(|client| client.client.websocket.close(/*msg*/ None));
            let _ = timeout(SHUTDOWN_TIMEOUT, try_join_all(closes)).await;
            if !matches!(state.child.try_wait(), Ok(Some(_))) {
                let _ = state.child.start_kill();
                let _ = timeout(SHUTDOWN_TIMEOUT, state.child.wait()).await;
            }
        });
    }
}

struct HostState {
    child: Child,
    client: ClientState,
}

struct MultiClientHostState {
    child: Child,
    clients: Vec<ClientState>,
}

struct ClientState {
    client: HostClient,
    session_id: SessionId,
    payload: String,
    next_request_number: i64,
}

#[derive(Clone, Copy)]
enum BatchWorkload {
    EchoPayload,
    SmallOutput,
    TrivialCell,
}

impl HostState {
    async fn start(payload_bytes: usize) -> Result<Self> {
        let (child, websocket_url) = start_host().await?;
        let client =
            ClientState::connect(&websocket_url, "throughput-benchmark", payload_bytes).await?;
        Ok(Self { child, client })
    }

    async fn run_batch(&mut self, batch_size: usize, workload: BatchWorkload) -> Result<()> {
        self.client.run_batch(batch_size, workload).await
    }
}

impl MultiClientHostState {
    async fn start(client_count: usize, payload_bytes: usize) -> Result<Self> {
        let (child, websocket_url) = start_host().await?;
        let clients = try_join_all((0..client_count).map(|client_index| {
            ClientState::connect(
                &websocket_url,
                format!("throughput-benchmark-{client_index}"),
                payload_bytes,
            )
        }))
        .await?;
        Ok(Self { child, clients })
    }

    async fn run_batch(&mut self, batch_size: usize) -> Result<()> {
        timeout(
            BATCH_TIMEOUT,
            try_join_all(
                self.clients
                    .iter_mut()
                    .map(|client| client.run_batch_inner(batch_size, BatchWorkload::EchoPayload)),
            ),
        )
        .await
        .context("timed out running multi-client code-mode WebSocket benchmark batch")??;
        Ok(())
    }
}

impl ClientState {
    async fn connect(
        websocket_url: &str,
        session_name: impl Into<String>,
        payload_bytes: usize,
    ) -> Result<Self> {
        let mut client = HostClient::connect(websocket_url).await?;
        let hello = ClientHello::new(
            SupportedProtocolVersions::try_new([ProtocolVersion::V1])?,
            CapabilitySet::empty(),
            CapabilitySet::empty(),
        )?;
        client
            .exchange(
                ClientToHost::ClientHello(hello),
                HostToClient::HostHello(HostHello::new(
                    ProtocolVersion::V1,
                    CapabilitySet::empty(),
                )),
            )
            .await?;
        let session_id = SessionId::new(session_name)?;
        let mut state = Self {
            client,
            session_id,
            payload: "x".repeat(payload_bytes),
            next_request_number: 1,
        };
        let open_id = state.next_request_id();
        state
            .client
            .exchange(
                ClientToHost::Request {
                    id: open_id,
                    request: HostRequest::OpenSession {
                        session_id: state.session_id.clone(),
                    },
                },
                HostToClient::Response {
                    id: open_id,
                    result: WireResult::Ok {
                        value: HostResponse::SessionReady {
                            session_id: state.session_id.clone(),
                        },
                    },
                },
            )
            .await?;
        Ok(state)
    }

    async fn run_batch(&mut self, batch_size: usize, workload: BatchWorkload) -> Result<()> {
        timeout(BATCH_TIMEOUT, self.run_batch_inner(batch_size, workload))
            .await
            .context("timed out running code-mode WebSocket benchmark batch")?
    }

    async fn run_batch_inner(
        &mut self,
        batch_size: usize,
        workload: BatchWorkload,
    ) -> Result<()> {
        let requires_delegate = !matches!(workload, BatchWorkload::TrivialCell);
        let mut pending = HashMap::with_capacity(batch_size);
        let payload_json = serde_json::to_string(&self.payload)?;
        let echo_tool = WireToolDefinition {
            name: "echo".to_string(),
            tool_name: WireToolName {
                name: "echo".to_string(),
                namespace: None,
            },
            description: String::new(),
            kind: WireToolKind::Function,
            input_schema: None,
            output_schema: None,
        };

        for _ in 0..batch_size {
            let request_number = self.next_request_number;
            let request_id = self.next_request_id();
            let source = match workload {
                BatchWorkload::EchoPayload => format!(
                    "const result = await tools.echo({{ sequence: {request_number}, payload: {payload_json} }}); text(result.payload);"
                ),
                BatchWorkload::SmallOutput => format!(
                    "await tools.echo({{ sequence: {request_number}, payload: {payload_json} }}); text(\"ok\");"
                ),
                BatchWorkload::TrivialCell => "text(\"ok\");".to_string(),
            };
            self.client
                .send(&ClientToHost::Request {
                    id: request_id,
                    request: HostRequest::Execute {
                        session_id: self.session_id.clone(),
                        request: WireExecuteRequest {
                            tool_call_id: format!("throughput-bench-{request_number}"),
                            enabled_tools: if requires_delegate {
                                vec![echo_tool.clone()]
                            } else {
                                Vec::new()
                            },
                            source,
                            yield_time_ms: Some(EXECUTION_YIELD_TIME_MS),
                            max_output_tokens: Some(MAX_OUTPUT_TOKENS),
                        },
                    },
                })
                .await?;
            pending.insert(request_id, PendingExecution::default());
        }

        // A close can interleave before another frame identifies which request owns its cell.
        let mut routes = CellRoutes {
            requests: HashMap::with_capacity(batch_size),
            unmatched_closes: HashSet::with_capacity(batch_size),
        };
        while !pending
            .values()
            .all(|execution| {
                execution.started
                    && (execution.delegated || !requires_delegate)
                    && execution.initial
                    && execution.closed
            })
        {
            match self.client.read().await? {
                HostToClient::Response {
                    id,
                    result:
                        WireResult::Ok {
                            value: HostResponse::ExecutionStarted { cell_id },
                        },
                } => {
                    record_cell(&mut pending, &mut routes, id, &cell_id)?;
                    let execution = pending
                        .get_mut(&id)
                        .context("unknown execute response ID")?;
                    ensure!(!execution.started, "duplicate execute response");
                    execution.started = true;
                }
                HostToClient::DelegateRequest {
                    id,
                    session_id,
                    request: DelegateRequest::InvokeTool { invocation },
                } => {
                    ensure!(requires_delegate, "unexpected delegate request");
                    ensure!(session_id == self.session_id, "wrong delegate session");
                    let input = invocation
                        .input
                        .as_ref()
                        .context("missing delegate input")?;
                    let request_number = input
                        .get("sequence")
                        .and_then(Value::as_i64)
                        .context("missing delegate sequence")?;
                    let request_id = RequestId::new(request_number);
                    let expected_input = json!({
                        "sequence": request_number,
                        "payload": self.payload.as_str(),
                    });
                    ensure!(input == &expected_input, "wrong delegate input");
                    ensure!(
                        invocation.tool_name.name == "echo"
                            && invocation.tool_name.namespace.is_none()
                            && invocation.tool_kind == WireToolKind::Function,
                        "wrong delegate tool"
                    );
                    record_cell(&mut pending, &mut routes, request_id, &invocation.cell_id)?;
                    let execution = pending
                        .get_mut(&request_id)
                        .context("unknown delegate request ID")?;
                    ensure!(!execution.delegated, "duplicate delegate request");
                    execution.delegated = true;
                    self.client
                        .send(&ClientToHost::DelegateResponse {
                            id,
                            result: WireResult::Ok {
                                value: DelegateResponse::ToolResult {
                                    result: match workload {
                                        BatchWorkload::EchoPayload => json!({
                                            "sequence": request_number,
                                            "payload": self.payload.as_str(),
                                        }),
                                        BatchWorkload::SmallOutput => json!({
                                            "sequence": request_number,
                                        }),
                                        BatchWorkload::TrivialCell => unreachable!(
                                            "trivial-cell workload does not dispatch delegates"
                                        ),
                                    },
                                },
                            },
                        })
                        .await?;
                }
                HostToClient::InitialResponse {
                    id,
                    result:
                        WireResult::Ok {
                            value:
                                WireRuntimeResponse::Result {
                                    cell_id,
                                    content_items,
                                    error_text,
                                },
                        },
                } => {
                    let expected = vec![WireContentItem::InputText {
                        text: match workload {
                            BatchWorkload::EchoPayload => self.payload.clone(),
                            BatchWorkload::SmallOutput | BatchWorkload::TrivialCell => {
                                "ok".to_string()
                            }
                        },
                    }];
                    ensure!(content_items == expected && error_text.is_none(), "wrong initial response");
                    record_cell(&mut pending, &mut routes, id, &cell_id)?;
                    let execution = pending
                        .get_mut(&id)
                        .context("unknown initial response ID")?;
                    ensure!(!execution.initial, "duplicate initial response");
                    execution.initial = true;
                }
                HostToClient::CellClosed {
                    session_id,
                    cell_id,
                } => {
                    ensure!(session_id == self.session_id, "wrong cell-close session");
                    if let Some(request_id) = routes.requests.get(&cell_id).copied() {
                        let execution = pending
                            .get_mut(&request_id)
                            .context("unknown cell-close request ID")?;
                        ensure!(!execution.closed, "duplicate cell-close frame");
                        execution.closed = true;
                    } else {
                        ensure!(
                            routes.unmatched_closes.insert(cell_id),
                            "duplicate cell-close frame"
                        );
                    }
                }
                HostToClient::Response {
                    id,
                    result: WireResult::Err { message },
                }
                | HostToClient::InitialResponse {
                    id,
                    result: WireResult::Err { message },
                } => bail!("request {id:?} failed: {message}"),
                message => bail!("unexpected WebSocket message: {message:?}"),
            }
        }

        ensure!(routes.unmatched_closes.is_empty(), "cell-close for unknown cell");
        Ok(())
    }

    fn next_request_id(&mut self) -> RequestId {
        let request_id = RequestId::new(self.next_request_number);
        self.next_request_number += 1;
        request_id
    }
}

async fn start_host() -> Result<(Child, String)> {
    let host_program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut command = Command::new(host_program);
    command
        .args(["--listen", "ws://127.0.0.1:0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().context("failed to start code-mode host")?;
    let stdout = child
        .stdout
        .take()
        .context("code-mode host stdout was not captured")?;
    let mut lines = BufReader::new(stdout).lines();
    let websocket_url = timeout(IO_TIMEOUT, lines.next_line())
        .await
        .context("timed out waiting for code-mode host WebSocket URL")??
        .context("code-mode host exited before publishing its WebSocket URL")?;
    ensure!(
        websocket_url.starts_with("ws://127.0.0.1:"),
        "non-loopback URL"
    );
    Ok((child, websocket_url))
}

#[derive(Default)]
struct PendingExecution {
    cell_id: Option<WireCellId>,
    started: bool,
    delegated: bool,
    initial: bool,
    closed: bool,
}

struct CellRoutes {
    requests: HashMap<WireCellId, RequestId>,
    unmatched_closes: HashSet<WireCellId>,
}

fn record_cell(
    pending: &mut HashMap<RequestId, PendingExecution>,
    routes: &mut CellRoutes,
    request_id: RequestId,
    cell_id: &WireCellId,
) -> Result<()> {
    let execution = pending.get_mut(&request_id).context("unknown request ID")?;
    if let Some(expected) = &execution.cell_id {
        ensure!(expected == cell_id, "mismatched cell ID");
    } else {
        ensure!(routes.requests.insert(cell_id.clone(), request_id).is_none(), "reused cell ID");
        execution.cell_id = Some(cell_id.clone());
        execution.closed = routes.unmatched_closes.remove(cell_id);
    }
    Ok(())
}

struct HostClient {
    websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl HostClient {
    async fn connect(websocket_url: &str) -> Result<Self> {
        let (websocket, _) = timeout(IO_TIMEOUT, connect_async(websocket_url))
            .await
            .context("timed out connecting to code-mode host WebSocket")??;
        Ok(Self { websocket })
    }

    async fn exchange(&mut self, message: ClientToHost, expected: HostToClient) -> Result<()> {
        self.send(&message).await?;
        ensure!(self.read().await? == expected, "unexpected setup response");
        Ok(())
    }

    async fn send(&mut self, message: &ClientToHost) -> Result<()> {
        let frame = EncodedFrame::encode(message)?;
        timeout(
            IO_TIMEOUT,
            self.websocket
                .send(Message::Binary(frame.into_framed_bytes().into())),
        )
        .await
        .context("timed out writing code-mode WebSocket message")?
        .context("failed to write code-mode WebSocket message")
    }

    async fn read(&mut self) -> Result<HostToClient> {
        loop {
            let message = timeout(IO_TIMEOUT, self.websocket.next())
                .await
                .context("timed out waiting for code-mode WebSocket message")?
                .context("code-mode WebSocket closed before returning a message")?
                .context("failed to read code-mode WebSocket message")?;
            match message {
                Message::Binary(bytes) => {
                    return EncodedFrame::decode_framed(&bytes).context("invalid WebSocket frame");
                }
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(frame) => bail!("WebSocket closed: {frame:?}"),
                Message::Text(text) => bail!("unexpected text frame: {text}"),
                Message::Frame(_) => bail!("unexpected raw WebSocket frame"),
            }
        }
    }
}
