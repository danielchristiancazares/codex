# Performance Log

## Windows app-server delta notification mapping — current baseline

This section describes retained PERF-013 on top of signed commit `3aba0e58e9`. Core's agent-message, plan, reasoning-summary, and reasoning-content delta events already own their thread and turn IDs. The app-server protocol mapper previously allocated replacement IDs from its authoritative caller arguments before dispatching every event. The retained path compares the embedded IDs with those arguments and transfers matching strings into the notification. A mismatch follows the established caller-authoritative allocation path.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| App-server agent-message delta notification mapping | 76.62 ns/event | 39.77 ns/event | 48.09% less time; 1.93× throughput | 25.14 M events/s |

### Fixture and command

- Command: `just bench -- app_server_agent_delta_notification_mapping` from the repository root.
- Path: public production `item_event_to_server_notification` mapping from an owned Core `AgentMessageContentDeltaEvent` to the v2 `AgentMessageDeltaNotification` payload.
- Input: a matching 43-byte thread ID, 41-byte turn and item IDs, and a 16-byte delta. Divan clones the complete input event outside each timed invocation.
- Baseline operation: allocate and copy the caller's thread and turn IDs, move the item ID and delta into the notification, and discard the event's duplicate owned IDs.
- Retained operation: compare each event ID with its authoritative caller ID, move both matching event strings plus the item ID and delta into the notification, and retain allocation-backed caller IDs for mismatch cases.
- Sampling: 100 Divan samples × 1,000 iterations per invocation. One full rebuild-and-run invocation per source state is excluded, followed by five independently launched warmed invocations.
- Environment: Windows 11 Pro 10.0.26200, AMD Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and the optimized workspace benchmark profile described below.
- Metric boundary: local Core-event to typed-notification mapping and ownership transfer. Subscriber lookup, transport serialization, provider execution, and model generation lie outside this fixture.

### Raw baseline medians

| Run | Median per event | Derived throughput |
|---:|---:|---:|
| Warmup (excluded) | 77.57 ns | 12.89 M events/s |
| 1 | 76.27 ns | 13.11 M events/s |
| 2 | 76.62 ns | 13.05 M events/s |
| 3 | 76.42 ns | 13.08 M events/s |
| 4 | 78.47 ns | 12.74 M events/s |
| 5 | 77.66 ns | 12.88 M events/s |

Median of the five warmed invocation medians: **76.62 ns/event** and **13.05 M events/s**.

### Raw retained-state medians

| Run | Median per event | Derived throughput |
|---:|---:|---:|
| Final-state warmup (excluded) | 40.77 ns | 24.52 M events/s |
| 1 | 40.72 ns | 24.56 M events/s |
| 2 | 39.77 ns | 25.14 M events/s |
| 3 | 35.97 ns | 27.80 M events/s |
| 4 | 38.22 ns | 26.16 M events/s |
| 5 | 49.01 ns | 20.40 M events/s |

Median of the five exact-final warmed invocation medians: **39.77 ns/event** and **25.14 M events/s**. Relative to the 76.62 ns baseline, this is **48.09% less mapping time** and **92.61% more event throughput**. The exact-final 35.97–49.01 ns invocation-median range is disjoint from the 76.27–78.47 ns baseline range.

### Retained win

- **PERF-013 — transfer matching delta event IDs into typed notifications.** Agent-message, plan, reasoning-summary, and reasoning-content delta mappings share the same ownership rule. Embedded IDs that equal the caller's authoritative IDs move directly into the notification. Stale or inconsistent event IDs fall back independently to the caller's thread and turn IDs, preserving the established wire payload for every input. All item IDs, delta text, indexes, notification variants, method names, and serialization remain unchanged.

### Correctness and validation

- `just test -p codex-app-server-protocol`: all 300 tests passed with one pre-existing skipped test on the exact final source.
- The new whole-payload test covers all four optimized delta variants plus a stale embedded-ID case, confirming caller-authoritative fallback behavior.
- `just fix -p codex-app-server-protocol --lib --no-deps` and `just fix -p codex-core --bench latency_paths --no-deps`: passed. The first pass exposed a large-`Err` Clippy warning in an intermediate helper shape; the final early-match implementation passes cleanly.
- `cargo build -p codex-exec --release`: passed after rebuilding the complete in-process app-server path from the retained source.
- The exact-final component benchmark completed one excluded warmup and five recorded invocations with fully disjoint baseline and retained invocation-median ranges.
- The argument-comment source wrapper remains unavailable on this host because `cargo-dylint` and `dylint-link` are absent; the prebuilt recipe is Unix-only. This candidate adds no opaque positional-literal call site.

### Process-level diagnostic

- The 20,000-delta production-path WebSocket fixture reconstructed all 20,000 characters, emitted completion, and exited successfully in every ordinary sample. Its server delivered the 1,040,538-byte frame burst in 0.146–0.257 ms during the principal batches.
- A pre-edit batch measured a 359.211 ms response-to-render median and 55,677.57 delta events/s. Two retained-logic batches before the final source-shape cleanup measured 348.927 and 355.939 ms medians. Two exact-final batches then entered the fixture's slow scheduling mode at 613.523 and 623.073 ms.
- Side-by-side release binaries were alternated against one server instance. A 10-pair run produced 548.385 ms baseline and 649.239 ms candidate medians; paired candidate-minus-baseline values ranged from -235.916 to +654.032 ms. A 30-pair run produced 463.620 ms baseline and 603.606 ms candidate medians; paired deltas ranged from -135.643 ms to a 19,506.094 ms candidate pause, with 17 negative and 13 positive values.
- The fixture's multi-modal scheduling and extreme pauses dominate the sub-microsecond allocation change. These process measurements are classified as correctness and scheduler diagnostics. The isolated production mapper fixture supplies the attributable performance evidence for PERF-013.

### Rejected experiments since PERF-012

- **PERF-014 — exec notification opt-outs.** Configuring exec to opt out of the five agent, plan, and reasoning delta notifications it already ignores moved the process median from 359.211 to 357.064 ms, a 0.60% rate increase with overlapping samples. The source edit was reverted.
- **PERF-015 — early app-server notification suppression.** Propagating connection opt-outs into listener capabilities and skipping notification-only handling when every subscriber opted out produced 367.269 and 358.128 ms candidate medians against the 359.211 ms baseline. The source and focused test edits were reverted.
- **PERF-016 — inline Core response mapping.** Removing Core's second 1,600-entry response channel and mapper task produced a 610.692 ms process median during one slow-mode batch. Later exact-binary interleaving established the fixture's 0.35-second to multi-second scheduler modes, so that batch provides no channel-specific attribution. The change also replaced eager pipelining with consumer-driven mapping and removed a backpressure trace state. The source and test edits were reverted. Reopening this path requires a stable fixture that preserves and measures transport/consumer overlap.

## Windows app-server owned-event handoff — current baseline

This section describes retained PERF-012 on top of signed commit `724bec936f`. The per-thread app-server listener borrows each Core event for cost tracking, turn-state projection, raw-event policy, and realtime effects. It previously deep-cloned the event for the sole consuming bespoke handler because a shutdown-variant check followed that call. The retained path captures the shutdown condition by reference and transfers the original event directly.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| App-server owned handoff for an agent-message delta event | 193.4 ns/event | 64.37 ns/event | 66.72% less time; 3.00× throughput | 15.54 M events/s |

### Fixture and command

- Command: `just bench -- app_server_event_owned_handoff` from the repository root.
- Path: a focused component fixture reproducing the ownership operations immediately around `apply_bespoke_event_handling` in the production thread listener.
- Input: one `EventMsg::AgentMessageContentDelta` with a correlation ID plus owned thread, turn, item, and 16-byte delta strings. Divan constructs a fresh input clone outside each timed invocation.
- Baseline operation: classify the borrowed event for shutdown, deep-clone the complete event for the consuming handoff, black-box and drop the clone, then drop the original input.
- Retained operation: classify the borrowed event for shutdown, black-box and consume the original event directly, then retain the classification bit.
- Sampling: 100 Divan samples × 1,000 iterations per invocation. One full rebuild-and-run invocation per source state is excluded, followed by five independently launched warmed invocations. A separate pre-gate candidate set corroborated the final state with a 64.62 ns median.
- Environment: Windows 11 Pro 10.0.26200, AMD Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and the optimized workspace benchmark profile described below.
- Metric boundary: local event ownership transfer and destruction. Turn-state tracking, notification mapping, serialization, transport writes, provider execution, and model generation lie outside this fixture.

### Raw baseline medians

| Run | Median per event | Derived throughput |
|---:|---:|---:|
| Warmup (excluded) | 199.5 ns | 5.013 M events/s |
| 1 | 190.6 ns | 5.247 M events/s |
| 2 | 202.1 ns | 4.948 M events/s |
| 3 | 190.1 ns | 5.260 M events/s |
| 4 | 193.7 ns | 5.163 M events/s |
| 5 | 193.4 ns | 5.171 M events/s |

Median of the five warmed invocation medians: **193.4 ns/event** and **5.171 M events/s**.

### Raw retained-state medians

| Run | Median per event | Derived throughput |
|---:|---:|---:|
| Final-state warmup (excluded) | 70.07 ns | 14.27 M events/s |
| 1 | 64.37 ns | 15.54 M events/s |
| 2 | 67.52 ns | 14.81 M events/s |
| 3 | 64.12 ns | 15.60 M events/s |
| 4 | 63.57 ns | 15.73 M events/s |
| 5 | 68.02 ns | 14.70 M events/s |

Median of the five exact-final warmed invocation medians: **64.37 ns/event** and **15.54 M events/s**. Relative to the 193.4 ns baseline, this is **66.72% less ownership-handoff time** and **3.00× stage throughput**. The exact-final 63.57–68.02 ns invocation range is disjoint from the 190.1–202.1 ns baseline range.

### Retained win

- **PERF-012 — transfer the listener's owned event directly.** Every pre-handler observer retains the same borrowed event and ordering. Raw-response filtering still occurs before downstream work. The listener records `ShutdownComplete` as a boolean immediately before the consuming handler, awaits the handler exactly once, then resolves the shutdown-drain waiter at the same post-handler point. Typed notification payloads and ordering remain unchanged.

### Correctness and validation

- `just test -p codex-app-server --lib -E 'not test(/collect_resume_override_mismatches_includes_service_tier/)'`: all 284 relevant app-server library tests passed with one explicit exclusion. This covers 27 bespoke-event tests, thread state/status, realtime history, outgoing messages, shutdown behavior, listener FIFO behavior, and in-process delivery.
- The unfiltered 285-test library run passed 284 tests and found one deterministic, unrelated service-tier wording mismatch: the implementation produced `requested=fast`, while its fixture expected `requested=priority`. The exact exclusion and reopen condition are recorded in `FAILED_PATHS.md` under VALIDATION-003.
- The v2 plan-item integration probe reached normal listener notifications, then its legacy mock model provider hit the fork's WebSocket-only guard on both retries. `FAILED_PATHS.md` records this under VALIDATION-002.
- `just clippy -p codex-app-server --lib --no-deps` and `just clippy -p codex-core --bench latency_paths --no-deps`: passed; both repository recipes also compiled their test targets.
- `just fix -p codex-app-server --lib --no-deps` and `just fix -p codex-core --bench latency_paths --no-deps`: passed without reported edits.
- `just bench-smoke`: passed every registered benchmark target, including the owned-event fixture.
- `cargo build -p codex-exec --release`: passed on the retained source and linked the in-process app-server path.
- The argument-comment source wrapper remains unavailable on this host because `cargo-dylint` and `dylint-link` are absent; the prebuilt recipe is Unix-only. This candidate adds no opaque positional-literal call site.

## Windows WebSocket client-metadata handoff — current baseline

This section describes retained PERF-003 on top of signed commit `34cc98b7b3`. Each Responses request already owns a complete `client_metadata` map. The WebSocket path previously rebuilt that map independently, then `ResponseCreateWsRequest::from(&request)` deep-cloned the request-owned map into a temporary field that the struct update immediately replaced. The retained path moves the request-owned map at the final payload boundary and enriches that allocation in place.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| Responses request metadata handoff into a WebSocket create payload | 1.255 µs/request | 485.8 ns/request | 61.29% less time; 2.58× throughput | 2.058 M requests/s |

### Fixture and command

- Command: `just bench -- websocket_request_metadata_handoff` from the repository root.
- Path: a focused component fixture reproducing the ownership and conversion operations at the production `ResponsesApiRequest` → `ResponseCreateWsRequest` boundary.
- Input: one prebuilt request with eight realistic metadata entries: installation, session, thread, turn, window, serialized turn metadata, Responses Lite, and sticky turn state. A traceparent and tracestate exercise the existing W3C enrichment path. Divan prepares a fresh request clone outside each timed invocation.
- Baseline operation: deep-clone the request metadata for the explicit WebSocket field, invoke the cross-crate `ResponseCreateWsRequest::from` conversion that deep-clones the same map into its base value, replace and drop that temporary base field, then drop the resulting payload.
- Retained operation: take the existing map from the request, invoke the same W3C enrichment, let the base conversion observe an empty metadata option, then drop the resulting payload.
- Conservative boundary: production previously called `CodexResponsesMetadata::client_metadata()` a second time, including turn-metadata JSON serialization. The fixture represents that second construction as a map clone, so the measured improvement is a lower bound for the production request path.
- Sampling: 100 Divan samples × 1,000 iterations per invocation. One full rebuild-and-run invocation per source state is excluded, followed by five independently launched warmed invocations. A separate pre-gate candidate set corroborated the final state with a 457.9 ns median.
- Environment: Windows 11 Pro 10.0.26200, AMD Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and the optimized workspace benchmark profile described below.
- Metric boundary: local WebSocket request assembly. Request serialization, compression, socket transit, provider execution, and model generation lie outside this fixture.

### Raw baseline medians

| Run | Median per request | Derived throughput |
|---:|---:|---:|
| Warmup (excluded) | 1.248 µs | 801.3 K requests/s |
| 1 | 1.254 µs | 797.4 K requests/s |
| 2 | 1.282 µs | 780.0 K requests/s |
| 3 | 1.270 µs | 787.4 K requests/s |
| 4 | 1.255 µs | 796.8 K requests/s |
| 5 | 1.254 µs | 797.4 K requests/s |

Median of the five warmed invocation medians: **1.255 µs/request** and **796.8 K requests/s**.

### Raw retained-state medians

| Run | Median per request | Derived throughput |
|---:|---:|---:|
| Final-state warmup (excluded) | 496.5 ns | 2.014 M requests/s |
| 1 | 485.8 ns | 2.058 M requests/s |
| 2 | 488.2 ns | 2.048 M requests/s |
| 3 | 491.2 ns | 2.036 M requests/s |
| 4 | 455.1 ns | 2.197 M requests/s |
| 5 | 449.5 ns | 2.225 M requests/s |

Median of the five exact-final warmed invocation medians: **485.8 ns/request** and **2.058 M requests/s**. Relative to the 1.255 µs baseline, this is **61.29% less request-assembly time** and **2.58× stage throughput**. The exact-final 449.5–491.2 ns invocation range is disjoint from the 1.254–1.282 µs baseline range.

### Retained win

- **PERF-003 — move existing client metadata into the WebSocket payload.** Request telemetry, incremental-reuse comparison, response-item preparation, and the warmup inference-trace path all consume the original request before the ownership transfer. The payload then takes the map, adds Responses Lite and sticky-turn entries in place, adds W3C trace fields through the established helper, and serializes through the unchanged wire type. `last_request` retains every field used by the exhaustive WebSocket reuse comparator, which deliberately excludes per-response metadata.

### Correctness and validation

- `just test -p codex-core -E 'test(/responses_metadata_includes_window_lineage_and_turn_metadata/)'`: the focused metadata test passed and compared installation, session, thread, turn, window, parent-thread, subagent, and serialized turn-metadata values.
- `just test -p codex-core -E 'test(/client_websockets/)'`: all 44 WebSocket integration tests passed. Coverage includes ordinary metadata, caller-supplied metadata, per-turn traceparent/tracestate, Responses Lite toggling, preconnect and prewarm, incremental creates, canonical turn metadata, post-error full creates, connection-limit recovery, and session-cache reuse.
- `just clippy -p codex-core --lib --bench latency_paths --no-deps`: passed; the repository recipe also covered Core tests.
- `just fix -p codex-core --lib --bench latency_paths --no-deps`: passed without reported edits; the repository recipe also covered Core tests.
- `just bench-smoke`: passed every registered benchmark target, including the metadata-handoff fixture.
- `cargo build -p codex-exec --release`: passed on the retained source.
- The argument-comment source wrapper remains unavailable on this host because `cargo-dylint` and `dylint-link` are absent; the prebuilt recipe is Unix-only. Manual review found no new opaque positional-literal call site.

## Windows post-first-token TTFT gate — current baseline

This section describes PERF-004 on top of signed commit `1c39bfc048`. Core records turn time-to-first-token from the first eligible response event. The timestamp itself remains protected by the turn-timing mutex. Once that timestamp exists, later output-text, reasoning-summary, and reasoning-content deltas previously acquired the same mutex only to rediscover that TTFT was already fixed. The retained path uses an acquire/release atomic hint to return before that post-first-token lock.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| TTFT gate after the first eligible event | 26.05 ns/event | 0.2369 ns/event | 99.09% less time; 110.0× throughput | 4.219 G events/s |

### Fixture and command

- Command: `just bench -- turn_ttft_post_first_gate` from the repository root.
- Path: a focused component fixture mirroring the synchronization operation used by `TurnTimingState::record_ttft_for_response_event` after TTFT has been recorded. Production behavior is validated separately through the private `TurnTimingState` methods and a complete Core telemetry turn.
- Input: 100,000 sequential post-first-token observations in one current-thread Tokio runtime entry. The state begins in the recorded condition and is black-boxed at each observation.
- Baseline operation: acquire the uncontended `tokio::sync::Mutex`, read the recorded condition, release the guard, and continue.
- Retained operation: acquire-load the atomic recorded hint and take the established early-return branch.
- Sampling: 100 Divan samples × one 100,000-event batch per invocation. Runtime entry is outside the per-event loop, so its fixed cost is amortized equally. One full rebuild-and-run invocation per source state is excluded, followed by five independently launched warmed invocations.
- Environment: Windows 11 Pro 10.0.26200, AMD Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and the optimized workspace benchmark profile described below.
- Metric boundary: local post-first-token TTFT synchronization. Event classification, surrounding response handling, provider execution, and network transit lie outside this fixture.

### Raw baseline medians

| Run | Median per 100,000 events | Derived per-event time | Reported throughput |
|---:|---:|---:|---:|
| Warmup (excluded) | 2.609 ms | 26.09 ns | 38.32 M events/s |
| 1 | 2.619 ms | 26.19 ns | 38.16 M events/s |
| 2 | 2.605 ms | 26.05 ns | 38.37 M events/s |
| 3 | 2.600 ms | 26.00 ns | 38.45 M events/s |
| 4 | 2.664 ms | 26.64 ns | 37.52 M events/s |
| 5 | 2.601 ms | 26.01 ns | 38.43 M events/s |

Median of the five warmed invocation medians: **2.605 ms per 100,000 events**, **26.05 ns/event**, and **38.37 M events/s**.

### Raw retained-state medians

| Run | Median per 100,000 events | Derived per-event time | Reported throughput |
|---:|---:|---:|---:|
| Final-state warmup (excluded) | 23.89 µs | 0.2389 ns | 4.184 G events/s |
| 1 | 23.49 µs | 0.2349 ns | 4.255 G events/s |
| 2 | 23.69 µs | 0.2369 ns | 4.219 G events/s |
| 3 | 23.69 µs | 0.2369 ns | 4.219 G events/s |
| 4 | 23.64 µs | 0.2364 ns | 4.228 G events/s |
| 5 | 23.69 µs | 0.2369 ns | 4.219 G events/s |

Median of the five exact-final warmed invocation medians: **23.69 µs per 100,000 events**, **0.2369 ns/event**, and **4.219 G events/s**. Relative to the 26.05 ns baseline, this is **99.09% less synchronization time** and **110.0× stage throughput**. The exact-final invocation range of 0.2349–0.2369 ns/event is disjoint from the baseline range of 26.00–26.64 ns/event. The earlier retained-source set had a 23.79 µs median and independently corroborates the result.

### Retained win

- **PERF-004 — bypass the TTFT mutex after the first eligible response event.** `TurnTimingState` now publishes a recorded hint only after the mutex-protected timestamp is established. The first eligible event in each turn follows the original locked path, concurrent contenders serialize through the timestamp, and `mark_turn_started` clears both the timestamp and hint while holding the state lock. Ineligible events retain their existing classifier-only path. TTFT duration, TTFM state, item timing, profile timing, and telemetry emission preserve their established sources and units.

### Correctness and validation

- `just test -p codex-core -E 'test(/turn_timing_state/)'`: all 6 focused timing tests passed. Coverage includes no pre-start recording, once-per-turn behavior, reset on the next turn, independent TTFM recording, and a 32-way concurrent first-event race that produces exactly one TTFT duration.
- `just test -p codex-core -E 'test(/process_sse_emits_completed_telemetry/)'`: the complete Core turn passed and emitted response-completion telemetry with TTFT populated.
- `cargo build -p codex-exec --release`: passed on the retained source and produced the executable used for the secondary process check.
- `just clippy -p codex-core --lib --bench latency_paths --no-deps`: passed; the repository recipe also covered Core tests.
- `just fix -p codex-core --lib --bench latency_paths --no-deps`: passed without reported edits; the repository recipe also covered Core tests.
- `just bench-smoke`: passed every registered benchmark target, including the final TTFT fixture.
- `python tools/argument-comment-lint/run.py --help`: the source wrapper reported that `cargo-dylint` and `dylint-link` are unavailable on this host. The prebuilt recipe is Unix-only. Manual review found no new call site requiring an argument-name comment under the repository convention.

### Rejected and diagnostic measurements

- **Process-level 20,000-delta WebSocket burst:** a raw loopback WebSocket server modeled the v2 `generate=false` prewarm followed by the real request, sent a 1,040,538-byte uncompressed frame burst, and timed response start through the final JSON-mode agent item. Every run reconstructed all 20,000 `~` characters, emitted completion, exited successfully, and spent at most 0.618 ms in the server send. Baseline invocation medians were 425.039, 420.905, 431.140, 661.903, and 403.620 ms; retained-state medians were 597.220, 616.288, 361.912, 395.621, and 606.675 ms. Both states showed distinct fast and slow scheduling modes with heavily overlapping 361.912–661.903 ms invocation ranges and a large fixed turn-finalization floor. This fixture provides correctness evidence and no attributable performance conclusion for PERF-004.
- **Bazel exec macrobenchmark:** `bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed --test_arg=text_delta_response_to_exit //codex-rs/exec:codex-exec-bench` stopped during analysis because the maintainer-owned Cargo update resolved `prost-types` 0.14.4 while `bazel/toolchains/prost` still names 0.14.3. The benchmark-only source edit was fully reverted.
- **SSE process fixture:** the release binary rejected the custom SSE provider as incompatible with this WebSocket-only build before issuing a request. The production-aligned WebSocket fixture replaced it.

## Windows metrics-enabled WebSocket telemetry — current baseline

This section describes PERF-001E on top of signed commit `9b13f5bc27`. The Responses WebSocket decoder already constructs an owned `ResponsesStreamEvent` for every successful text frame. Metrics-enabled telemetry previously decoded the same frame again into a generic `serde_json::Value` to recover its `type`. The retained path hands the primary decoder's event kind and original payload to telemetry, preserving the payload parser for the single server-timing event while ordinary response deltas proceed directly to metric recording.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| WebSocket telemetry, metrics active, 85-byte text delta | 2.301 µs/event | 1.763 µs/event | 23.38% less time; 1.31× throughput | 567.1 K events/s |

### Fixture and command

- Command: `just bench -- websocket_telemetry_metrics_text_delta` from the repository root.
- Path: public production `SessionTelemetry` WebSocket-event methods with an active `MetricsClient` backed by `InMemoryMetricExporter`.
- Input: one 85-byte `response.output_text.delta` text frame carrying a 16-byte delta and sequence number 42, with a recorded socket-poll duration of 100 µs. The telemetry object, frame, and parsed metadata are constructed outside timing and black-boxed inside each iteration.
- Baseline operation: parse the already protocol-decoded frame again into `serde_json::Value`, allocate the event-kind `String`, build metric tags, and record the event counter and duration.
- Retained operation: borrow the event kind produced by the primary protocol decoder, build the same metric tags, and record the same event counter and duration. The original payload remains available for established server-timing extraction.
- Sampling: 100 Divan samples × 100 iterations per invocation. One full build-and-run invocation per exact source state is excluded, followed by five independently launched warmed invocations.
- Environment: Windows 11 Pro 10.0.26200, AMD Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and the optimized workspace benchmark profile described below.
- Metric boundary: local metrics-enabled telemetry processing after socket delivery. Provider execution and network transit lie outside this fixture.

### Raw baseline medians

| Run | Median | Reported event throughput | Reported byte throughput |
|---:|---:|---:|---:|
| Warmup (excluded) | 2.304 µs/event | 433.9 K events/s | 36.88 MB/s |
| 1 | 2.294 µs/event | 435.8 K events/s | 37.04 MB/s |
| 2 | 2.300 µs/event | 434.6 K events/s | 36.94 MB/s |
| 3 | 2.301 µs/event | 434.5 K events/s | 36.93 MB/s |
| 4 | 2.314 µs/event | 432.0 K events/s | 36.72 MB/s |
| 5 | 2.322 µs/event | 430.6 K events/s | 36.60 MB/s |

Median of the five warmed invocation medians: **2.301 µs/event**, **434.5 K events/s**, and **36.93 MB/s**.

### Raw retained-state medians

An initial candidate shape retained the decoded timing object in `ResponsesStreamEvent` and produced a corroborating 1.683 µs median. The exact final shape keeps `timing_metrics` opaque to the protocol schema and carries the original payload by borrowed reference, preserving duplicate-field handling. Its first release rebuild serves as the excluded warmup below.

| Run | Median | Reported event throughput | Reported byte throughput |
|---:|---:|---:|---:|
| Exact-final-source warmup (excluded) | 1.803 µs/event | 554.3 K events/s | 47.12 MB/s |
| 1 | 1.815 µs/event | 550.8 K events/s | 46.82 MB/s |
| 2 | 1.763 µs/event | 567.1 K events/s | 48.20 MB/s |
| 3 | 1.741 µs/event | 574.1 K events/s | 48.79 MB/s |
| 4 | 1.751 µs/event | 570.8 K events/s | 48.52 MB/s |
| 5 | 1.783 µs/event | 560.5 K events/s | 47.65 MB/s |

Median of the five exact-final-source invocation medians: **1.763 µs/event**, **567.1 K events/s**, and **48.20 MB/s**. Relative to the 2.301 µs baseline, this is **23.38% less telemetry time** and **30.52% more event throughput**. The retained invocation range of 1.741–1.815 µs is disjoint from the baseline range of 2.294–2.322 µs.

### Retained win

- **PERF-001E — reuse decoded WebSocket event metadata in active telemetry.** `WebsocketTelemetry` now offers a defaulted parsed-event callback, preserving existing implementors through the raw callback. The production Core adapter overrides it and records metrics from the borrowed event kind. The endpoint captures socket-poll duration at the established boundary, decodes each successful text frame once, and invokes raw telemetry for schema-invalid text, transport outcomes, and non-text frames. Server timing events parse their original payload through the established generic JSON path. Wrapped-error precedence, `response.failed` tagging, ping/pong suppression, runtime counters, and timing summaries retain their established behavior.

### Correctness and validation

- `just test -p codex-api`: all 186 tests passed on the exact final source, including the 10 Responses WebSocket endpoint cases.
- `just test -p codex-otel`: all 61 tests passed, including raw event recording, parsed server-timing extraction, and the complete runtime summary.
- `just test -p codex-core responses_websocket_emits_websocket_telemetry_events`: passed through the production dynamic callback and loopback transport.
- `just test -p codex-core responses_websocket_includes_timing_metrics_header_when_runtime_metrics_enabled`: passed all six extracted timing values through the production parsed-event path.
- The exact-final optimized benchmark completed one excluded warmup and five recorded invocations with fully disjoint baseline and retained ranges.
- `just bench-smoke`: passed every registered workspace benchmark, including both WebSocket telemetry configurations and the persistent production-path loopback fixture.
- `just fix -p codex-api`, `just fix -p codex-otel`, `just fix -p codex-core --lib --no-deps`, and `just fix -p codex-core --bench latency_paths --no-deps`: passed.
- The Windows justfile has no `argument-comment-lint` recipe. New opaque numeric arguments use exact parameter comments.

## Windows transient rollout append — current baseline

This section describes PERF-011 on top of signed commit `3439fdfb9e`. Core sends streaming agent, reasoning, and plan deltas through `LiveThread::append_items`, where the shared rollout policy rejects them from durable history. `LiveThread` already constructs that filtered batch for metadata projection. The retained path also uses it for the backing-store append, removing a second raw-delta clone and redundant policy pass while preserving store lifecycle checks and persistence telemetry.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| `LiveThread` transient agent-message delta append | 402.2 ns/event | 238.2 ns/event | 40.78% less time; 1.69× throughput | 4.197 M events/s |

### Fixture and command

- Command: `just bench -- live_thread_transient_delta_append` from the repository root.
- Path: public production `LiveThread::append_items`, shared rollout filtering, and the production `InMemoryThreadStore` append boundary on a current-thread Tokio runtime.
- Input: one legacy-history `AgentMessageContentDelta` with three IDs and a 16-byte delta. The event and `LiveThread` are constructed outside timing.
- Precondition and barriers: the fixture asserts that global persistence metrics are absent and that the shared rollout policy classifies the input as transient. It black-boxes the `LiveThread`, item slice, and complete async result on every iteration.
- Baseline operation: filter and clone retained items inside `LiveThread`, clone the original raw batch into the backing-store request, and filter it again inside the backing store.
- Retained operation: filter once, clone the filtered batch into the backing-store request, and retain the existing metadata projection. For this transient event, the backing store receives an empty batch and performs its established empty-append path.
- Sampling: 100 Divan samples × 1,000 iterations per invocation. One full build-and-run invocation per source state is excluded, followed by five independently launched warmed invocations.
- Environment: Windows 11 Pro 10.0.26200, AMD Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and the optimized workspace benchmark profile described below.

### Raw baseline medians

| Run | Median | Reported event throughput |
|---:|---:|---:|
| Warmup (excluded) | 404.4 ns/event | 2.472 M events/s |
| 1 | 399.5 ns/event | 2.502 M events/s |
| 2 | 419.4 ns/event | 2.383 M events/s |
| 3 | 403.0 ns/event | 2.480 M events/s |
| 4 | 402.2 ns/event | 2.485 M events/s |
| 5 | 401.7 ns/event | 2.488 M events/s |

Median of the five warmed invocation medians: **402.2 ns/event**, or **2.485 M events/s**.

### Raw retained-state medians

The first candidate set produced warmed medians of 236.1, 243.6, 240.9, 242.0, and 238.8 ns/event, yielding a corroborating 240.9 ns median. The final fixture adds two assertions outside the timed loop. Its first invocation rebuilt the downstream benchmark graph and serves as the exact-final-source warmup.

| Run | Median | Reported event throughput |
|---:|---:|---:|
| Exact-final-source warmup (excluded) | 237.8 ns/event | 4.203 M events/s |
| 1 | 237.1 ns/event | 4.217 M events/s |
| 2 | 238.2 ns/event | 4.197 M events/s |
| 3 | 238.1 ns/event | 4.199 M events/s |
| 4 | 242.7 ns/event | 4.119 M events/s |
| 5 | 241.5 ns/event | 4.140 M events/s |

Median of the five exact-final-source invocation medians: **238.2 ns/event**, or **4.197 M events/s**. Relative to the 402.2 ns baseline, this is **40.78% less transient-append time** and **1.69× event throughput**. The retained invocation range of 237.1–242.7 ns is disjoint from the baseline range of 399.5–419.4 ns.

### Retained win

- **PERF-011 — reuse the filtered `LiveThread` append batch.** `LiveThread::persist_appended_items` now sends the already-filtered items to `ThreadStore::append_items`. Transient response deltas therefore avoid a deep `EventMsg` clone and a second meaningful filter pass. Mixed batches preserve durable-item order and metadata observation. Local backing stores still receive empty append calls, retaining defensive `ThreadNotFound` behavior for writes racing with discard or shutdown. Raw items remain available to rollout persistence telemetry before recording the batch.

### Correctness and validation

- `just test -p codex-thread-store --test live_thread`: 2 tests passed. The integration target compares the complete serialized history after a mixed transient/durable append and verifies that a transient append after local-store discard still returns the exact missing thread ID.
- `just test -p codex-thread-store live_thread::tests` reached four maintainer-owned unit-test compile gaps where the active `reasoning_mode` protocol change has not yet populated new fields. The task-owned assertion compile issue from that attempt was corrected by comparing complete serialized histories. The public-API integration target compiles the production library independently of those unrelated unit-test fixtures.
- `just bench-smoke`: passed every registered workspace benchmark, including the retained fixture.
- `just clippy -p codex-core --bench latency_paths --no-deps`: passed for the task-owned benchmark and compiled the changed `codex-thread-store` library dependency.
- The Windows justfile has no `argument-comment-lint` recipe. New opaque numeric arguments use exact parameter comments.
- `just fix -p codex-core --bench latency_paths --no-deps`: passed for the task-owned benchmark target.
- The crate-wide `just fix -p codex-thread-store` gate shares the four pre-existing unit-test compile errors recorded above, so it is unavailable until those maintainer-owned fixtures are updated. The changed production library compiled cleanly through the integration, benchmark, smoke, and Clippy paths.
- `just fmt`: passed as the final formatting gate.

## Windows app-server notification opt-out lookup — current baseline

This section describes PERF-010 on top of signed commit `bceef00ef8`. App-server checks each typed notification against the destination connection's opt-out set before enqueueing it. Streaming agent, reasoning, and plan deltas can therefore visit this lookup once per destination connection and per delta. The retained path borrows the notification method's static wire name through the existing enum metadata.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| App-server notification opt-out miss, agent-message delta | 39.02 ns/lookup | 9.92 ns/lookup | 74.58% less time; 3.93× throughput | 100.8 M lookups/s |

### Fixture and command

- Command: `just bench -- server_notification_opt_out_lookup` from the repository root.
- Path: the exact notification method-name conversion and `HashSet<String>::contains` operation used by `should_skip_notification_for_connection`.
- Input: one `ServerNotification::AgentMessageDelta` carrying three IDs and a 16-byte delta. The connection opt-out set contains the distinct explicit wire method `item/reasoning/textDelta`, exercising the ordinary lookup-miss path.
- Baseline operation: convert the notification to a newly allocated `String`, then hash and query its borrowed `str`.
- Retained operation: borrow the notification's static wire name with `AsRef<str>`, then hash and query it directly.
- Barriers: the benchmark constructs the notification and opt-out set outside the timed loop and black-boxes both references inside every iteration.
- Sampling: 100 Divan samples × 1,000 iterations per invocation. One full invocation per source state is excluded, followed by five independently launched warmed invocations.
- Environment: Windows 11 Pro 10.0.26200, AMD Ryzen 9 9900X, rustc 1.98.0, High performance power scheme, and the optimized workspace benchmark profile described below.

### Raw baseline medians

| Run | Median | Reported lookup throughput |
|---:|---:|---:|
| Warmup (excluded) | 40.12 ns/lookup | 24.92 M lookups/s |
| 1 | 38.92 ns/lookup | 25.69 M lookups/s |
| 2 | 39.12 ns/lookup | 25.56 M lookups/s |
| 3 | 38.32 ns/lookup | 26.09 M lookups/s |
| 4 | 39.02 ns/lookup | 25.62 M lookups/s |
| 5 | 39.82 ns/lookup | 25.11 M lookups/s |

Median of the five warmed invocation medians: **39.02 ns/lookup**, or **25.62 M lookups/s**.

### Raw retained-state medians

An initial candidate set produced warmed medians of 10.12, 9.92, 10.02, 9.92, and 9.82 ns/lookup. After correctness and lint validation, the exact final source was measured again from a fresh excluded warmup.

| Run | Median | Reported lookup throughput |
|---:|---:|---:|
| Exact-final-source warmup (excluded) | 9.82 ns/lookup | 101.8 M lookups/s |
| 1 | 9.82 ns/lookup | 101.8 M lookups/s |
| 2 | 9.92 ns/lookup | 100.8 M lookups/s |
| 3 | 9.82 ns/lookup | 101.8 M lookups/s |
| 4 | 10.01 ns/lookup | 99.90 M lookups/s |
| 5 | 9.92 ns/lookup | 100.8 M lookups/s |

Median of the five exact-final-source invocation medians: **9.92 ns/lookup**, or **100.8 M lookups/s**. Relative to the 39.02 ns baseline, this is **74.58% less lookup time** and **3.93× lookup throughput**. The retained invocation range of 9.82–10.01 ns is disjoint from the baseline range of 38.32–39.82 ns.

### Retained win

- **PERF-010 — borrow notification method names during opt-out filtering.** The macro-generated `ServerNotification` enum now derives `AsRefStr` from the same Strum camel-case and explicit per-variant wire-name metadata already used by `Display`. App-server queries the opt-out set with that borrowed static name, eliminating one `String` allocation, method-name copy, and deallocation from each typed-notification lookup. Serialization, `Display`, experimental gating, and connection routing retain their established behavior.

### Correctness and validation

- `just test -p codex-app-server-protocol`: 299 tests passed and 1 was skipped, including the generated stable and experimental schema fixture gates.
- `just test -p codex-app-server to_connection_notification_respects_opt_out_filters`: passed. The focused transport test opts out of `item/agentMessage/delta`, directly proving that `AsRef<str>` honors an explicitly renamed wire method. The sibling default-name test continues to cover `configWarning`.
- `just bench-smoke`: passed every registered workspace benchmark, including the retained fixture.
- `just clippy -p codex-core --bench latency_paths --no-deps`: passed for the task-owned benchmark target and its dependency graph.
- The Windows justfile has no `argument-comment-lint` recipe. The new opaque numeric argument uses the exact `/*count*/` parameter comment.
- `just fix -p codex-app-server-protocol` and `just fix -p codex-app-server`: passed.
- `just fmt`: passed as the final formatting gate.

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

1. **PERF-004:** avoid post-first-token TTFT mutex acquisitions on later deltas.
2. **PERF-006:** reject non-durable delta events before persistent thread-store work.
3. **PERF-007:** collapse redundant app-server delta fanout work.

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

This section describes PERF-008 and PERF-009 on top of signed commit `ff5b6f6ea5`. Core calls `SessionTelemetry::record_responses` for every decoded response event before turn processing. The benchmark isolates its ordinary output-text-delta path with tracing disabled, which is a common release configuration and the state in which the avoidable name allocation dominated this stage. Enabled spans retain the same label values and also avoid the static-label allocation.

### Current timings

| Benchmark | Baseline | Current | Delta | Throughput |
|---|---:|---:|---:|---:|
| Response telemetry, tracing-disabled output-text delta | 21.22 ns/event | 1.32 ns/event | 93.78% less time; 16.08× throughput | 757.5 M events/s |

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

### PERF-008 retained-state / PERF-009 baseline medians

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

### PERF-009 retained-state medians

The first post-edit invocation rebuilt downstream benchmark crates in 1m 40s and served as the excluded warmup.

| Run | Median | Reported event throughput |
|---:|---:|---:|
| Warmup (excluded) | 1.32 ns/event | 757.5 M events/s |
| 1 | 1.32 ns/event | 757.5 M events/s |
| 2 | 1.32 ns/event | 757.5 M events/s |
| 3 | 1.32 ns/event | 757.5 M events/s |
| 4 | 1.32 ns/event | 757.5 M events/s |
| 5 | 1.32 ns/event | 757.5 M events/s |

Median of the five warmed invocation medians: **1.32 ns/event**, or **757.5 M events/s**. Relative to the immediate 1.72 ns PERF-008 baseline, this is **23.26% less time** and **30.31% more event throughput**. Relative to the original 21.22 ns baseline, the cumulative result is **93.78% less time** and **16.08× event throughput**.

### Retained wins

- **PERF-008 — borrow static response telemetry names.** Response and response-item classifiers now return `Cow<'static, str>`. Every static event label, including the per-delta `text_delta`, remains borrowed through synchronous span recording. The sole dynamic `message_from_{role}` label keeps its owned formatted representation. All emitted `otel.name` values remain unchanged.
- **PERF-009 — skip disabled response spans at entry.** `record_responses` now returns immediately when its span is disabled. Such a span discards every field recording operation; the guard also skips event classification and the second event match. Enabled spans proceed through the established label and metadata path.

### Correctness and validation

- `just test -p codex-otel`: all 61 tests passed.
- `just test -p codex-core record_responses_sets_span_fields_for_response_events`: passed an integration stream covering `created`, `rate_limits`, function calls, the dynamic `message_from_assistant` label, reasoning, text/reasoning deltas, and completion.
- `just bench-smoke`: passed every registered workspace benchmark, including the new fixture.
- `just clippy -p codex-core --bench latency_paths --no-deps`: passed for the task-owned benchmark target.
- `just fix -p codex-otel` and final `just fmt`: passed.
- The Windows justfile has no `argument-comment-lint` recipe. The new opaque numeric argument uses the exact `/*count*/` parameter comment.
- PERF-009 `just test -p codex-otel`: all 61 tests passed on the guarded source.
- PERF-009 `just test -p codex-core record_responses_sets_span_fields_for_response_events`: passed the enabled-span integration stream and every exact static/dynamic field assertion.
- PERF-009 `just bench-smoke`, `just fix -p codex-otel`, and final `just fmt`: passed.

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

1. **PERF-003:** eliminate the deep client-metadata clone that WebSocket request assembly immediately replaces.
2. **PERF-004:** avoid post-first-token TTFT mutex acquisitions on later eligible deltas.
3. **PERF-006:** reject non-durable delta events before cloning or persistent thread-store submission where the current event pipeline still permits it.
4. **PERF-007:** collapse redundant app-server delta-event clones, state locking, ID allocation, and notification construction along the active inference consumer path.
