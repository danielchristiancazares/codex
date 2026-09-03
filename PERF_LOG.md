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

1. **PERF-001B:** remove the ordinary-frame wrapped-error parse from the production WebSocket decoder.
2. **PERF-001C:** construct bounded raw protocol diagnostics only when conversion fails.
3. **PERF-001D:** share the primary event discriminator with metrics-enabled WebSocket telemetry.
4. **PERF-004:** avoid post-first-token TTFT mutex acquisitions on later deltas.

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
