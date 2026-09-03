# Performance Log

## Windows cloud inference telemetry — current baseline

This section describes the retained source state based on `cdee9db6662a437fb66d5cad962b4eca703334bd`, plus the task-owned benchmark and metrics-absence fast path. The worktree also contains pre-existing maintainer changes outside this performance work.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| WebSocket telemetry, metrics absent, 85-byte text delta | 395.7 ns/frame | 1.92 ns/frame | 99.51% less time; 206.1× throughput | 520.8 Mframe/s |

### Why this benchmark is production-relevant

Ordinary cloud inference uses the Responses WebSocket path in this fork. Core always supplies its WebSocket telemetry adapter, which calls `SessionTelemetry::record_websocket_event` for every received frame. The retained method now checks its optional metrics sink at entry and returns immediately when metrics are absent. The benchmark calls this production method directly and isolates the per-frame client cost from loopback-network and server scheduling variance.

### Environment and command

- Host: AMD Ryzen 9 9900X, 12 cores / 24 logical processors, 64 GiB RAM.
- OS: Windows 11 Pro 10.0.26200.
- Power scheme: High performance (`8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c`).
- Toolchain: rustc 1.98.0 (`88d9e12ae178fab0fb5cc050a94da85685d449ea`), host `x86_64-pc-windows-msvc`, LLVM 22.1.8.
- Build profile: Cargo benchmark profile, optimized with line-table debuginfo; workspace release settings use LTO off, 16 codegen units, and incremental compilation.
- Command, run from the repository root: `just bench -- websocket_telemetry_no_metrics_text_delta`.
- Expanded recipe: `cargo bench --message-format short --workspace --bench '*'` with the Divan name filter.
- Fixture: one 85-byte `response.output_text.delta` WebSocket text message with a 16-byte delta and sequence number 42; metrics client absent; recorded poll duration 100 µs. The benchmark asserts that the global metrics client is absent, then black-boxes both the telemetry receiver and response on every timed invocation.
- Sampling: 100 Divan samples × 1,000 iterations per invocation.
- Warmup: one full build-and-run invocation, excluded from the aggregate.

### Raw baseline medians

| Run | Median | Reported item throughput | Reported byte throughput |
|---:|---:|---:|---:|
| Warmup (excluded) | 389.7 ns/frame | 2.566 Mframe/s | 218.1 MB/s |
| 1 | 406.8 ns/frame | 2.457 Mframe/s | 208.9 MB/s |
| 2 | 395.7 ns/frame | 2.527 Mframe/s | 214.7 MB/s |
| 3 | 393.5 ns/frame | 2.541 Mframe/s | 216.0 MB/s |
| 4 | 403.5 ns/frame | 2.478 Mframe/s | 210.6 MB/s |
| 5 | 389.5 ns/frame | 2.567 Mframe/s | 218.2 MB/s |

Median of the five warmed invocation medians: **395.7 ns/frame**.

### Raw retained-state medians

The first post-edit invocation rebuilt downstream benchmark crates in 1m 53s and served as the excluded warmup. Its measured median was 1.92 ns/frame.

| Run | Median | Reported item throughput | Reported byte throughput |
|---:|---:|---:|---:|
| 1 | 1.92 ns/frame | 520.8 Mframe/s | 44.27 GB/s |
| 2 | 1.92 ns/frame | 520.8 Mframe/s | 44.27 GB/s |
| 3 | 1.92 ns/frame | 520.8 Mframe/s | 44.27 GB/s |
| 4 | 1.91 ns/frame | 523.5 Mframe/s | 44.50 GB/s |
| 5 | 1.92 ns/frame | 520.8 Mframe/s | 44.27 GB/s |

Median of the five warmed invocation medians: **1.92 ns/frame**. Relative to the 395.7 ns baseline, this is a **99.51% reduction in per-frame time** and **206.1× throughput** for the metrics-disabled telemetry stage.

### Retained wins

- **PERF-001A — metrics-absence fast path.** `SessionTelemetry::record_websocket_event` returns at its boundary when the session has no metrics client. This removes a full `serde_json::Value` decode, event-kind allocation, tag construction, and two inactive metric-helper calls from every received WebSocket frame in that configuration. Metrics-enabled event classification and timing extraction follow the existing path. The applicability includes standalone app-server, VS Code/remote-control, analytics-disabled configurations, and builds whose exporter resolves to disabled; default release-TUI analytics commonly provide an active metrics client and continue through the existing metrics path.

### Active candidates

1. **PERF-001E:** share the primary event discriminator with metrics-enabled WebSocket telemetry.
2. **PERF-004:** avoid post-first-token TTFT mutex acquisitions on later deltas.
3. **PERF-006:** reject non-durable delta events before persistent thread-store work.
4. **PERF-007:** collapse redundant app-server delta fanout work.

### Rejected experiments

- **BENCH-001 — HTTP SSE as the primary inference benchmark.** Ordinary cloud inference is WebSocket-only in this fork. The provisional one-chunk SSE fixture also amplified `eventsource-stream` tail copying and failed to compile because its host benchmark crate had no direct `bytes` dependency. The benchmark-only edit was removed before this baseline.
- **BENCH-002 — receiver-visible telemetry microbenchmark.** The first fixture black-boxed the response alone, leaving the known metrics-empty telemetry receiver visible to future cross-crate optimization. The benchmark was hardened before retention by asserting its global-metrics precondition and black-boxing the receiver on every iteration. Both source states were remeasured from scratch; the preliminary 379.0 ns and 1.91 ns aggregates were superseded by the hardened dataset above.

### Validation state

- The benchmark compiled and completed in the optimized benchmark profile before and after the candidate.
- `just test -p codex-otel`: 61 tests passed on the final retained source state, including metrics-enabled WebSocket event and timing-metric coverage in the runtime summary suite.
- `just bench-smoke`: passed for all registered workspace benchmark targets.
- `just fix -p codex-otel`: passed.
- `just clippy -p codex-core --bench latency_paths --no-deps`: passed. The initial dependency-inclusive invocation reached an existing denied `expect()` in `codex-exec-output-artifacts/src/store.rs:225`; the narrowed run checked the task-owned benchmark crate while compiling its dependency graph.
- `just argument-comment-lint codex-rs/core/benches/latency_paths.rs`: unavailable because the Windows justfile does not define this Unix-only recipe. The new opaque numeric arguments carry exact `/*count*/` and `/*micros*/` parameter comments.
- `just fmt`: passed as the final formatting gate.

## Windows response span-name recording — current baseline

This section describes PERF-008 on top of signed commit `ff5b6f6ea5`. Core calls `SessionTelemetry::record_responses` for every decoded response event before turn processing. The benchmark isolates its ordinary output-text-delta path with tracing disabled, which is a common release configuration and the state in which the avoidable name allocation dominated this stage. Enabled spans retain the same label values and also avoid the static-label allocation.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| Response span-name recording, tracing-disabled output-text delta | 21.22 ns/event | 1.72 ns/event | 91.89% less time; 12.34× throughput | 581.3 M events/s |

### Fixture and command

- Command: `just bench -- response_telemetry_text_delta` from the repository root.
- Path: public production `SessionTelemetry::record_responses` with a `ResponseEvent::OutputTextDelta` and `tracing::Span::none()`.
- Precondition and barriers: the fixture asserts that the span is disabled, then black-boxes the telemetry receiver, span, and event on every timed invocation.
- Input: one 16-byte text delta. Telemetry metadata and the event are constructed outside the timed loop.
- Sampling: 100 Divan samples × 1,000 iterations per invocation. One full build-and-run invocation per source state is excluded, followed by five independently launched warmed invocations.
- Environment: same Windows 11, Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and optimized benchmark profile recorded above.

### Raw baseline medians

| Run | Median | Reported event throughput |
|---:|---:|---:|
| Warmup (excluded) | 21.52 ns/event | 46.46 M events/s |
| 1 | 21.22 ns/event | 47.12 M events/s |
| 2 | 21.82 ns/event | 45.82 M events/s |
| 3 | 21.62 ns/event | 46.25 M events/s |
| 4 | 21.17 ns/event | 47.23 M events/s |
| 5 | 21.02 ns/event | 47.57 M events/s |

Median of the five warmed invocation medians: **21.22 ns/event**, or **47.12 M events/s**.

### Raw retained-state medians

The final fixture adds an explicit disabled-span assertion outside the timed loop. Its first post-edit invocation rebuilt downstream benchmark crates in 4m 03s and served as the excluded candidate build warmup. A fresh benchmark-source rebuild after adding the assertion served as the exact-final-source warmup.

| Run | Median | Reported event throughput |
|---:|---:|---:|
| Exact-final-source warmup (excluded) | 1.72 ns/event | 581.3 M events/s |
| 1 | 1.72 ns/event | 581.3 M events/s |
| 2 | 1.72 ns/event | 581.3 M events/s |
| 3 | 1.72 ns/event | 581.3 M events/s |
| 4 | 1.72 ns/event | 581.3 M events/s |
| 5 | 1.72 ns/event | 581.3 M events/s |

Median of the five exact-final-source invocation medians: **1.72 ns/event**, or **581.3 M events/s**. Relative to the 21.22 ns baseline, this is **91.89% less time** and **12.34× event throughput** for response telemetry on a disabled span.

### Retained win

- **PERF-008 — borrow static response telemetry names.** Response and response-item classifiers now return `Cow<'static, str>`. Every static event label, including the per-delta `text_delta`, remains borrowed through synchronous span recording. The sole dynamic `message_from_{role}` label keeps its owned formatted representation. All emitted `otel.name` values remain unchanged.

### Correctness and validation

- `just test -p codex-otel`: all 61 tests passed.
- `just test -p codex-core record_responses_sets_span_fields_for_response_events`: passed an integration stream covering `created`, `rate_limits`, function calls, the dynamic `message_from_assistant` label, reasoning, text/reasoning deltas, and completion.
- `just bench-smoke`: passed every registered workspace benchmark, including the new fixture.
- `just clippy -p codex-core --bench latency_paths --no-deps`: passed for the task-owned benchmark target.
- `just fix -p codex-otel` and final `just fmt`: passed.
- The Windows justfile has no `argument-comment-lint` recipe. The new opaque numeric argument uses the exact `/*count*/` parameter comment.

## Windows Responses WebSocket event throughput — current baseline

This section begins at signed commit `b28d181e8f`, including PERF-001A, plus the task-owned production-path benchmark harness, PERF-001B, and PERF-001C. It measures the persistent Responses WebSocket event pipeline used by ordinary cloud inference. Server model execution sits outside this loopback saturation fixture.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| Persistent WebSocket response, 16,384 text deltas plus completion | 39.47 ms/response | 35.30 ms/response | 10.56% less time; 11.80% more throughput | 464.1 K events/s |

### Fixture and command

- Command: `just bench -- responses_websocket_16384_text_deltas` from the repository root.
- Path: public `ResponsesWebsocketClient::connect` and `ResponsesWebsocketConnection::stream_request`, the production metrics-absent per-frame telemetry behavior, WebSocket decompression and frame handling, event decoding/conversion, the production 1,600-entry response channel, and a consumer that drains through channel closure so parser-task teardown stays inside each sample.
- Transport: one persistent direct loopback WebSocket. A setup precondition verifies that Tungstenite's environment proxy resolution selects a direct route. The server handshake records and asserts negotiated `permessage-deflate` before timing begins.
- Runtime isolation: client pump, decoder, and consumer use one cooperative current-thread Tokio runtime. Server framing and deflate run on an independently owned one-worker runtime, removing artificial client/server executor contention.
- Response: 16,384 independently framed 85-byte `response.output_text.delta` messages plus one 183-byte `response.completed` message; 16,385 events and 1,392,823 decoded JSON payload bytes per sample. The repeated 16-byte delta yields 262,144 accumulated text bytes and intentionally amplifies parser throughput under a warmed compression dictionary.
- Correctness: every sample compares the complete `ResponseSummary`: 16,384 deltas, 262,144 accumulated delta bytes, matching completion ID, and zero unexpected events. The consumer treats duplicate completion events as unexpected and reaches the closed response channel before returning.
- Sampling: 50 Divan samples × one complete response per invocation. Every process performs one full untimed response warmup before sampling. One full build-and-run invocation per source state is excluded, followed by five independently launched warmed invocations.
- Environment: same Windows 11, Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and optimized benchmark profile recorded above.

### Raw baseline medians

| Run | Median | Reported event throughput | Reported payload throughput |
|---:|---:|---:|---:|
| Warmup (excluded) | 41.72 ms/response | 392.7 K events/s | 33.38 MB/s |
| 1 | 40.09 ms/response | 408.6 K events/s | 34.73 MB/s |
| 2 | 41.06 ms/response | 398.9 K events/s | 33.91 MB/s |
| 3 | 39.47 ms/response | 415.1 K events/s | 35.28 MB/s |
| 4 | 39.33 ms/response | 416.5 K events/s | 35.41 MB/s |
| 5 | 38.48 ms/response | 425.7 K events/s | 36.19 MB/s |

Median of the five warmed invocation medians: **39.47 ms/response**, or **415.1 K events/s**.

### PERF-001B retained-state / PERF-001C baseline medians

The first post-edit invocation rebuilt downstream benchmark crates in 5m 29s and served as the excluded warmup. Its measured median was 38.32 ms/response.

| Run | Median | Reported event throughput | Reported payload throughput |
|---:|---:|---:|---:|
| 1 | 38.34 ms/response | 427.2 K events/s | 36.32 MB/s |
| 2 | 36.66 ms/response | 446.8 K events/s | 37.98 MB/s |
| 3 | 36.67 ms/response | 446.8 K events/s | 37.98 MB/s |
| 4 | 38.13 ms/response | 429.6 K events/s | 36.52 MB/s |
| 5 | 38.26 ms/response | 428.2 K events/s | 36.40 MB/s |
| Final confirmation (outside aggregate) | 36.84 ms/response | 444.6 K events/s | 37.79 MB/s |

Median of the five warmed invocation medians: **38.13 ms/response**, or **429.6 K events/s**. Relative to the 39.47 ms baseline, this is **3.40% less response-processing time** and **3.49% more event throughput**. Every retained-state invocation median was below every baseline invocation median.

### PERF-001C retained-state medians

The first candidate-source invocation served as the excluded build warmup and measured 35.24 ms/response. Its five warmed invocation medians were 35.51, 36.80, 35.66, 35.12, and 35.07 ms/response, yielding a corroborating median of 35.51 ms/response and 461.4 K events/s. Clippy then identified a redundant `Option<&str>::as_deref` left by the first implementation shape. The final helper accepts `&str` directly; the exact final source was rebuilt and remeasured from a fresh excluded warmup.

| Run | Median | Reported event throughput | Reported payload throughput |
|---:|---:|---:|---:|
| Final-source rebuild warmup (excluded) | 36.69 ms/response | 446.5 K events/s | 37.96 MB/s |
| 1 | 42.34 ms/response | 386.9 K events/s | 32.89 MB/s |
| 2 | 35.12 ms/response | 466.4 K events/s | 39.65 MB/s |
| 3 | 34.75 ms/response | 471.4 K events/s | 40.07 MB/s |
| 4 | 35.30 ms/response | 464.1 K events/s | 39.45 MB/s |
| 5 | 35.94 ms/response | 455.8 K events/s | 38.74 MB/s |

Median of the five exact-final-source invocation medians: **35.30 ms/response**, or **464.1 K events/s**. Relative to the immediate PERF-001B baseline of 38.13 ms, this is **7.42% less response-processing time** and **8.03% more event throughput**. Relative to the original 39.47 ms parser-order baseline, the cumulative retained result is **10.56% less time** and **11.80% more event throughput**. Run 1 spanned a broad 35.59–51.36 ms intra-invocation range under host noise; the other four exact-final invocation medians were below every PERF-001B baseline median, and the independent first candidate set corroborated the result.

### Retained wins

- **PERF-001B — wrapped-error parse gating.** Ordinary successfully parsed frames use the general `ResponsesStreamEvent` decoder once and skip the specialized wrapped-error decoder when their top-level kind differs from `error`. Parsed `error` events still receive wrapped-error mapping before semantic event processing. General parse failures still invoke wrapped-error mapping first, preserving specialized HTTP/retry error precedence for schema-conflicting error bodies. A shared error-kind constant keeps both recognition sites aligned.
- **PERF-001C — lazy raw protocol diagnostics.** Successfully decoded SSE and WebSocket frames retain a borrow of their original payload through synchronous semantic conversion. The bounded diagnostic `String` is now allocated and copied only when semantic conversion returns a response-protocol error. Malformed JSON diagnostics and wrapped-error mapping preserve their existing behavior. This removes one allocation, payload copy, and deallocation from every successfully converted response event.

### Correctness coverage

- The invalid-request WebSocket integration fixture now includes a numeric `delta` field. General event decoding rejects that field while wrapped-error decoding accepts and maps the HTTP 400 body, directly guarding failure-path precedence.
- Existing unit coverage retains mapped 429, mapped invalid-request 400, retryable connection-limit 400, ignored ordinary events, and unmapped status-free error behavior.
- `just test -p codex-api`: 186 tests passed.
- `just test -p codex-core responses_websocket_invalid_request_error_with_status_is_forwarded`: passed the precedence fixture.
- `just test -p codex-core responses_websocket_connection_limit_error_reconnects_and_completes`: passed the retryable mapped-error path.
- `just test -p codex-core websocket_rate_limit_with_nested_retry_after_is_terminal`: passed the mapped 429 retry-after path.
- `just test -p codex-guardian-v2 sampler_retries_expired_websockets_on_another_warm_connection` and `sampler_reconnects_after_transient_service_failures`: passed both Guardian transport recovery paths.
- `just bench-smoke`: passed every registered workspace benchmark. A final optimized confirmation after correctness validation measured 36.84 ms/response and 444.6 K events/s.
- `just fix -p codex-api`: passed without changing the maintainer-owned `endpoint/models.rs` worktree edit.
- `just clippy -p codex-core --bench latency_paths --no-deps`: passed for the benchmark target and its task-owned harness.
- The Windows justfile has no `argument-comment-lint` recipe. New opaque numeric arguments use exact `/*val*/` and `/*secs*/` parameter comments.
- `just fmt`: passed as the final formatting gate.
- PERF-001C strengthened the protocol-schema test across all 12 model-visible event error shapes, including `response.incomplete`. The fixture places a four-byte UTF-8 scalar across the 2,048-byte diagnostic boundary and compares the complete bounded payload, directly preserving exact truncation behavior after delayed construction.
- PERF-001C `just test -p codex-api`: all 186 tests passed.
- PERF-001C targeted Core coverage passed `malformed_completed_is_terminal_and_preserves_completed_output`, `process_sse_failed_event_logs_response_completed_parse_error`, and `responses_websocket_invalid_request_error_with_status_is_forwarded`.
- PERF-001C `just clippy -p codex-api` and `just bench-smoke`: passed on the final source.
- PERF-001C `just fix -p codex-api` and final `just fmt`: passed.

### Rejected and superseded measurements

- **BENCH-003 — 4,096-event response fixture.** The first production-path fixture used 30 samples and produced baseline medians 10.31, 10.30, 10.64, 10.42, and 9.845 ms versus candidate medians 10.06, 9.398, 9.974, 9.865, and 9.746 ms. Its 4.32% median time reduction was directional while the invocation ranges overlapped. The 16,384-event fixture replaced it.
- **BENCH-004 — shared/two-worker loopback scheduling.** The initial amplified fixture shared a two-worker runtime between server deflate and client work, depended implicitly on proxy routing, stopped at the completion event, and inferred compression negotiation. Those measurements were discarded. After direct-route, runtime-isolation, compression, and drain hardening, two nominally identical two-worker client invocations still produced 47.67 and 38.94 ms medians. The retained current-thread client fixture removed that cross-worker placement mode and was rebaselined from the original production source.
- **PERF-001D — borrow the owned event discriminator during semantic conversion.** Removing the per-event `event.kind` clone produced an excluded 35.12 ms warmup and warmed invocation medians of 35.19, 35.24, 34.29, 35.39, and 35.23 ms/response. The 35.23 ms aggregate median was 0.20% below the immediate 35.30 ms PERF-001C baseline, with fully overlapping invocation ranges. The production edit was reverted exactly.

### Active candidates

1. **PERF-001E:** share the primary event discriminator with metrics-enabled WebSocket telemetry.
2. **PERF-003:** eliminate the deep client-metadata clone that WebSocket request assembly immediately replaces.
3. **PERF-004:** avoid post-first-token TTFT mutex acquisitions on later eligible deltas.
4. **PERF-006:** reject non-durable delta events before cloning or persistent thread-store submission where the current event pipeline still permits it.
5. **PERF-007:** collapse redundant app-server delta-event clones, state locking, ID allocation, and notification construction along the active inference consumer path.
