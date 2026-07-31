# Performance Log

Record performance experiments here so future work starts from measured results instead of
repeating discarded ideas.

## Benchmark rules

- Compare release-mode runs using the same benchmark command and fixtures.
- Warm the build before recording numbers, then run each comparison at least twice.
- Prefer medians; treat small changes inside run-to-run variance as noise.
- Keep a code change only when the improvement is repeatable and behavior remains covered by tests.
- `BlockingLruCache` is disabled outside a Tokio runtime. Benchmarks intended to measure cache hits
  must enter a runtime or they will silently measure the miss path.

## 2026-08-01: TUI large-thread resume critical path

Scope: end-to-end time from launching `codex resume <thread-id>` to scheduling the first TUI frame
for a large local thread. The retained change overlaps the app-server `thread/resume` request with
startup bootstrap and hook discovery when the resumed thread supplies its saved model settings.

Environment: Intel Xeon W-3223, 16 logical CPUs, macOS x86_64, Rust 1.95.0, Bazel 9.0.0. The
preserved pre-change binary was built from commit `a003a5a499b27689d992abb8a84f6c1a510b8059`.

### Methodology

The fixture was
`sessions/2026/07/24/rollout-2026-07-24T10-49-31-019f953f-2447-7960-a979-725ad1e4bf27.jsonl`:
116,304,980 bytes, 5,142 JSONL records, and 101 turns. A mode-0700 temporary `CODEX_HOME` held an
identical private copy of the fixture, config, model cache, and app-server state for both binaries.
Each launch used that same warmed home and repository. The final comparison alternated baseline
and final binaries; one final launch that emitted no startup timing was discarded and replaced.

Both binaries were built in release mode with:

```sh
bazel build --compilation_mode=opt //codex-rs/cli:codex
cp bazel-bin/codex-rs/cli/codex /private/tmp/codex-perf-final-prefetch
```

The baseline binary had been preserved as `/private/tmp/codex-perf-baseline-a003a5a`. Each sample
ran in the same PTY for five seconds, changing only the binary path:

```sh
/usr/bin/expect -c '
log_user 0
spawn env TERM=xterm-256color CODEX_HOME=/private/tmp/codex-resume-bench.y9kbpJ \
  /private/tmp/codex-perf-final-prefetch resume \
  019f953f-2447-7960-a979-725ad1e4bf27 -C /Users/daniel/codex
after 5000
send "\003"
after 500
send "\003"
after 500
catch close
catch wait
'
sqlite3 /private/tmp/codex-resume-bench.y9kbpJ/logs_2.sqlite \
  "select feedback_log_body from logs where feedback_log_body like \
  'tui startup initial frame scheduled%' order by id desc limit 1;"
```

### Current baseline

All values are milliseconds from the structured `tui startup initial frame scheduled` event. The
table reports the three final interleaved samples and their median.

| State | First-frame samples | Median | Bootstrap median | Thread/widget median | Initial-session median |
| --- | ---: | ---: | ---: | ---: | ---: |
| Pre-change | 2706 / 2454 / 2343 | 2454 | 622 | 1161 | 664 |
| Current final state | 1932 / 1882 / 1984 | 1932 | 499 | 52 | 717 |

The current final median is 522 ms lower: **21.3% less end-to-end first-frame latency and a 1.27x
speedup**. The sample ranges do not overlap (baseline 2343-2706 ms; final 1882-1984 ms). The
sequential thread/widget phase falls from a 1161 ms median to 52 ms because the resume request is
mostly hidden behind other startup work.

### Retained win

Normal persistent resumes now begin `thread/resume` through the existing app-server request handle
before startup bootstrap and hook discovery, and await all three with `tokio::join!`. The response
is completed through the same fork-parent title lookup and resumed-session construction as the
existing sequential path. Resumes with explicit model or provider overrides retain their existing
current-config request path.

The added async regression test resumes a real temporary rollout through the prefetched request,
then verifies the persisted user history, thread ID, and direct-input state.

### Rejected experiment

An initial candidate requested only the newest 100 turns and excluded the full turn payload from
the resume response. This 101-turn fixture omitted only one turn, and its 2.361 s and 2.225 s
samples overlapped the original 2.268-2.405 s baseline range. The candidate was reverted before
the retained concurrency change.

### Validation

```sh
env \
  V8_FROM_SOURCE=0 \
  V8_FORCE_DEBUG=0 \
  MACOSX_DEPLOYMENT_TARGET=12.0 \
  RUSTY_V8_ARCHIVE=/Users/daniel/rusty_v8/target/release/gn_out/obj/librusty_v8.a \
  RUSTY_V8_SRC_BINDING_PATH=/Users/daniel/rusty_v8/target/release/gn_out/src_binding.rs \
  just test -p codex-tui startup_resume_request_restores_persisted_history
env \
  V8_FROM_SOURCE=0 \
  V8_FORCE_DEBUG=0 \
  MACOSX_DEPLOYMENT_TARGET=12.0 \
  RUSTY_V8_ARCHIVE=/Users/daniel/rusty_v8/target/release/gn_out/obj/librusty_v8.a \
  RUSTY_V8_SRC_BINDING_PATH=/Users/daniel/rusty_v8/target/release/gn_out/src_binding.rs \
  just test -p codex-tui
env \
  V8_FROM_SOURCE=0 \
  V8_FORCE_DEBUG=0 \
  MACOSX_DEPLOYMENT_TARGET=12.0 \
  RUSTY_V8_ARCHIVE=/Users/daniel/rusty_v8/target/release/gn_out/obj/librusty_v8.a \
  RUSTY_V8_SRC_BINDING_PATH=/Users/daniel/rusty_v8/target/release/gn_out/src_binding.rs \
  just fix -p codex-tui
just fmt
bazel build --compilation_mode=opt //codex-rs/cli:codex
```

The focused resume test passed. The full TUI run completed all 3,316 tests: 3,293 passed and 23
version-bearing snapshots failed because this fork renders `0.146.0-daniel` while those fixtures
expect `0.0.0`; four additional tests were skipped. The scoped fixer, formatter, optimized build,
and final interleaved PTY benchmark completed successfully.

## 2026-08-01: TUI workspace-settings tail latency

Status after the 2026-08-17 upstream rebase: upstream commit `3666a46ea3` removed the workspace
settings gate for apps and plugins, so the hedged request described in this historical entry is no
longer present in the current tree.

Scope: end-to-end TUI time to the first rendered frame for ChatGPT workspace accounts. Startup
loads `hooks/list` before the main app so new or changed hooks can be reviewed. That request also
fetches the workspace plugin setting, making its HTTP tail latency part of the visible launch path.

Environment: Intel Xeon W-3223, 16 logical CPUs, macOS x86_64, Rust 1.95.0, Bazel 9.0.0. The
pre-change optimized binary was built from commit `d48e5dddfcfbb008433e02efd0f2733a60fbe1b7`.

### Methodology

Initial production-state measurements used a 116,304,980-byte, 5,142-record rollout. Two launches
scheduled the first frame in 2.406 s and 2.551 s. An identical launch took 11.889 s, with 10.107 s
inside bootstrap. Structured request logs localized the delay to the workspace-settings GET: the
request reached its existing ten-second deadline while model-list and local rollout work completed.

The retained comparison made that tail deterministic. A mode-0700 temporary `CODEX_HOME` used the
same workspace account, config, and warmed model cache in both states. A local threaded HTTP server
assigned one base URL to each paired sample (`/b1/backend-api`, `/b2/backend-api`, and
`/b3/backend-api`). For each base URL, the first in-flight
`/accounts/<account>/settings` request slept for 12 seconds; a concurrent request returned
`{"beta_settings":{"enable_plugins":true}}` immediately. Other routes returned HTTP 503. Each
launch ran in a PTY for the same 13 seconds, and the primary result came from the TUI's structured
`tui startup initial frame scheduled` timing.

The optimized binaries were warmed and built with:

```sh
bazel build --compilation_mode=opt //codex-rs/cli:codex
cp bazel-bin/codex-rs/cli/codex /private/tmp/codex-perf-baseline-d48e5dd
```

The paired launch command was repeated for `b1`, `b2`, and `b3`, changing only
`CODEX_BENCH_BIN` between the preserved baseline binary and `bazel-bin/codex-rs/cli/codex`:

```sh
export CODEX_BENCH_HOME=/private/tmp/codex-workspace-settings-bench.myKoyu
export CODEX_BENCH_BIN=/private/tmp/codex-perf-baseline-d48e5dd
export CODEX_BENCH_RUN=b1
/usr/bin/expect -c '
log_user 0
set base_url "http://127.0.0.1:39431/$env(CODEX_BENCH_RUN)/backend-api"
set override "chatgpt_base_url=\"$base_url\""
spawn env TERM=xterm-256color CODEX_HOME=$env(CODEX_BENCH_HOME) \
  $env(CODEX_BENCH_BIN) -C /Users/daniel/codex \
  -c $override
after 13000
send "\003"
after 200
send "\003"
after 1000
close
wait
'
sqlite3 "$CODEX_BENCH_HOME/logs_2.sqlite" \
  "select feedback_log_body from logs where feedback_log_body like \
  'tui startup initial frame scheduled%' order by id desc limit 3;"
```

### Current baseline

| State | First-frame samples | Median | Bootstrap median |
| --- | ---: | ---: | ---: |
| Pre-change | 10.307 / 10.073 / 10.068 s | 10.073 s | 10.068 s |
| Current final state | 1.299 / 1.067 / 1.068 s | 1.068 s | 1.062 s |

The current final median is 9.005 seconds lower: **89.4% less first-frame latency and a 9.4x
speedup** on the reproduced tail fixture.

As a healthy-service control, the preserved baseline scheduled the 116 MB rollout's first frame in
2.644 / 2.241 / 2.356 s (2.356 s median), and the final binary recorded 2.715 / 2.292 / 2.500 s
(2.500 s median). The ranges overlap. Each final `hooks/list` settings fetch completed with one
request, confirming that responses inside the one-second threshold keep the single-request path.

### Retained win

Workspace settings now starts one hedged GET when the initial request remains pending for one
second. The first successful response wins. An early error waits for the other in-flight attempt,
and one outer timeout keeps both attempts inside the original ten-second total budget. The existing
15-minute in-process cache and fresh workspace-policy lookup remain unchanged.

Three async correctness tests cover the stalled-primary win, the healthy single-request path, and
an immediately failed hedge followed by a successful primary request.

### Rejected experiments and designs

#### CLI and rollout parsing

`codex exec` startup was about 0.5 seconds and was rejected as too small to be meaningfully visible.
A temporary release example then measured all 5,142 records of the 116 MB rollout at
343.535-385.739 ms across six warmed parses. The whole phase bounded the possible win below one
second, so the example was removed and the parser was left unchanged.

#### Shorter workspace-settings timeout

Reducing the ten-second deadline would also reduce the observed stall, while increasing fail-open
workspace-policy decisions on slow successful services. The deadline remains ten seconds.

#### Cross-process workspace-policy cache

A disk cache would remove the request on warm launches and carry workspace policy across process
restarts. Existing behavior refreshes that policy after a restart, and app-server integration
coverage depends on observing a changed workspace setting in the next process. The cache remains
process-local.

### Validation

```sh
env \
  V8_FROM_SOURCE=0 \
  V8_FORCE_DEBUG=0 \
  MACOSX_DEPLOYMENT_TARGET=12.0 \
  RUSTY_V8_ARCHIVE=/Users/daniel/rusty_v8/target/release/gn_out/obj/librusty_v8.a \
  RUSTY_V8_SRC_BINDING_PATH=/Users/daniel/rusty_v8/target/release/gn_out/src_binding.rs \
  just test -p codex-chatgpt
bazel build --compilation_mode=opt //codex-rs/cli:codex
just fix -p codex-chatgpt
just fmt
```

All 12 `codex-chatgpt` tests passed. The optimized build, three paired PTY baseline launches, three
paired PTY final launches, and three healthy 116 MB resume controls completed successfully.

## 2026-08-01: TUI suffix diff scanning

Scope: `codex-tui` custom-terminal buffer diffing and ANSI serialization for the existing 120x40
rendering fixtures. The optimization retains the styled wide-glyph repair path while finding each
row's last visible cell from the right.

Environment: Intel Xeon W-3223, 16 logical CPUs, macOS x86_64, Rust 1.95.0. The pre-change
baseline was commit `e42dfa73e305971f958a88a8b0a5780e80b76844`.

### Methodology

The benchmark command was run once to warm the optimized build and twice more for each recorded
configuration:

```sh
cd codex-rs
cargo bench -q -p codex-tui --bench rendering -- --color never
```

All runs used the same existing Divan fixtures and sampling: 50 samples x 500 iterations for
unchanged, sparse-update, and hyperlink cases; 30 samples x 100 iterations for dense repaint.
The table reports the median of the two recorded run medians and their minimum-to-maximum range.

### Current baseline

| Benchmark | Pre-change median | Final median | Result |
| --- | ---: | ---: | ---: |
| `buffer_diff_unchanged` | 128.1 us (127.6-128.5) | 113.9 us (113.8-113.9) | 11.1% faster |
| `buffer_diff_sparse_update` | 127.8 us (127.7-127.8) | 113.8 us (113.7-113.9) | 10.9% faster |
| `buffer_diff_dense_repaint` | 173.2 us (172.3-174.1) | 153.5 us (153.4-153.6) | 11.4% faster |
| `ansi_sparse_update` | 128.1 us (128.0-128.2) | 114.2 us (114.0-114.3) | 10.9% faster |
| `ansi_hyperlink_update` | 128.0 us (127.9-128.0) | 113.7 us (113.4-113.9) | 11.2% faster |
| `ansi_dense_repaint` | 251.0 us (250.8-251.1) | 232.9 us (232.6-233.2) | 7.2% faster |

### Retained win

`diff_buffers` now precomputes the exact rare styled `ForcedWidth` repair predicate once and scans
each row from right to left to locate its last visible cell. This removes common full-row symbol,
width, and repair checks while preserving clear-to-end and wide-glyph behavior. The improvement was
repeatable across both recorded runs, with 7.2%-11.4% lower medians across the rendering fixtures.

### Validation

```sh
cd codex-rs
just test -p codex-tui
just test -p codex-tui custom_terminal::tests
just test -p codex-tui terminal_hyperlinks::tests
just fix -p codex-tui
just fmt
```

The focused custom-terminal (12 tests) and hyperlink (14 tests) suites passed. The full TUI run
reported 3292 passed, 23 failures, and 4 skipped; the failures were existing time/update/status
snapshot cases. The final `just fix` and `just fmt` passes completed successfully.

## 2026-08-01: TUI suffix-clear eligibility scans

Scope: `codex-tui` custom-terminal buffer diffing for the existing 120x40 rendering fixtures. The
optimization narrows the previous-buffer and current-buffer suffix checks to the displayed region
that can intersect `ClearToEnd`.

Environment: Intel Xeon W-3223, 16 logical CPUs, macOS x86_64, Rust 1.95.0. The pre-change
baseline was commit `238e038d16de203d7b7c9a56fe4342626db81320`.

### Methodology

The benchmark command was run three warmed times for each state after the build was ready. The
table reports the median of the three run medians and their minimum-to-maximum range:

```sh
cd codex-rs
cargo bench -q -p codex-tui --bench rendering -- --color never
```

All runs used the same existing Divan fixtures and sampling: 50 samples x 500 iterations for
unchanged, sparse-update, and hyperlink cases; 30 samples x 100 iterations for dense repaint.

### Current baseline

| Benchmark | Pre-change median | Final median | Result |
| --- | ---: | ---: | ---: |
| `buffer_diff_unchanged` | 113.6 us (113.6-113.8) | 98.22 us (98.08-98.53) | 13.5% faster |
| `buffer_diff_sparse_update` | 113.9 us (113.8-113.9) | 98.10 us (98.09-98.37) | 13.9% faster |
| `buffer_diff_dense_repaint` | 153.4 us (153.3-154.2) | 133.4 us (133.2-133.4) | 13.0% faster |
| `ansi_sparse_update` | 114.2 us (114.1-114.4) | 98.59 us (98.55-98.63) | 13.7% faster |
| `ansi_hyperlink_update` | 113.4 us (113.4-113.5) | 98.88 us (98.77-99.38) | 12.8% faster |
| `ansi_dense_repaint` | 233.0 us (232.7-233.1) | 211.5 us (210.4-213.0) | 9.2% faster |

### Retained win

The suffix-clear checks now scan from the right edge and stop after the displayed range ends before
the clear boundary. Zero-width empty trailing cells remain traversable so a preceding wide cell is
still considered, while zero-width symbols with content retain the existing one-column treatment.
The change preserves styled-cell clearing, `AlwaysUpdate`, wide-glyph, and hyperlink behavior.
The final three warmed runs show 9.2%-13.9% lower medians across every fixture.

### Validation

```sh
cd codex-rs
just test -p codex-tui custom_terminal::tests
just test -p codex-tui terminal_hyperlinks::tests
just test -p codex-tui
just fix -p codex-tui
just fmt
cargo bench -q -p codex-tui --bench rendering -- --color never
```

The focused custom-terminal (12 tests) and hyperlink (14 tests) suites passed. The full TUI run
reported 3292 passed, 23 failures, and 4 skipped; its failures were the existing time/update/status
snapshot cases. The scoped fixer, formatter, and final post-format benchmark completed
successfully.

## 2026-08-01: code-mode-host transport throughput

Scope: end-to-end `codex-code-mode-host` stdio and WebSocket execution throughput, with emphasis on
serving concurrent WebSocket clients without changing the stdio runtime model.

Environment: Intel Xeon W-3223, 16 logical CPUs, macOS x86_64, Rust 1.95.0, Bazel 9.0.0. The
comparison baseline commit was `ebf7d28aa1926c767646e40b2fc78e96b03b6fa9`.

### Fixtures and methodology

The Bazel-only Divan fixtures spawn the optimized host binary as a child, negotiate protocol v1,
open sessions, and complete an untimed warm-up execution before collecting samples. The matched
stdio and WebSocket cases execute eight concurrent cells with 64-byte payloads and four concurrent
cells with 8 KiB payloads. Additional WebSocket coverage measures one 64-byte cell sequentially,
eight clients each executing eight 64-byte cells, 12 same-connection delegates carrying 64 KiB
inputs with small outputs, and 32 concurrent trivial cells without delegates.

The WebSocket command was run once to warm the optimized build and then three more times for the
recorded results:

```sh
bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed \
  //codex-rs/code-mode-host:websocket-throughput-bench
```

The matched stdio command was likewise run once to warm the build and twice more for the recorded
results:

```sh
bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed \
  //codex-rs/code-mode-host:stdio-throughput-bench
```

The table reports the median of the recorded run medians and their minimum-to-maximum range. These
ranges are medians across repeated warmed runs, not confidence intervals. The excluded warm-up
command produced valid benchmark output but was used only to establish the optimized build state.

### Current baseline

| Transport | Benchmark fixture | Divan sampling | Median of run medians | Range of run medians |
| --- | --- | --- | ---: | ---: |
| WebSocket | `small_payload_batch` (8 x 64 B) | 20 x 1 | 5.186 ms | 5.013-5.207 ms |
| WebSocket | `large_payload_batch` (4 x 8 KiB) | 20 x 1 | 3.226 ms | 3.184-3.243 ms |
| WebSocket | `sequential_payload_round_trip` (1 x 64 B) | 50 x 1 | 1.658 ms | 1.651-1.673 ms |
| WebSocket | `multi_client_small_payload_round_trips` (8 clients x 8 x 64 B) | 20 x 1 | 25.66 ms | 25.52-25.85 ms |
| WebSocket | `concurrent_large_delegate_serialization` (12 x 64 KiB) | 20 x 1 | 7.925 ms | 7.892-8.320 ms |
| WebSocket | `concurrent_trivial_cells` (32 cells) | 30 x 1 | 16.00 ms | 15.93-16.13 ms |
| stdio | `small_delegate_batch` (8 x 64 B) | 20 x 1 | 5.814 ms | 5.779-5.849 ms |
| stdio | `large_delegate_batch` (4 x 8 KiB) | 20 x 1 | 3.434 ms | 3.286-3.581 ms |

### Retained win

The standalone binary now selects a two-worker Tokio runtime only for `ws://` listeners and retains
the current-thread runtime for stdio. The WebSocket comparison used three warmed optimized runs;
the multi-client before and after ranges did not overlap.

| Workload | Before | After | Result |
| --- | ---: | ---: | ---: |
| 8 clients x 8 small-payload cells | 42.00 ms | 25.68 ms | 38.86% faster |
| Sequential small-payload round trip | 1.633 ms | 1.650 ms | 1.04% slower |
| 8-cell small-payload batch | 5.490 ms | 5.064 ms | 7.76% faster |
| 4-cell large-payload batch | 3.381 ms | 3.263 ms | 3.49% faster |

The small sequential regression was accepted because it stayed near 1%, while the retained runtime
substantially improved the targeted multi-client workload and also improved both matched batch
cases. Stdio does not pay for a multi-thread runtime.

### Rejected experiments

#### Default Tokio stdio `BufReader` and `BufWriter`

Interleaved small-payload medians were A 5.844/6.124/6.049 ms versus B
5.770/5.424/5.639 ms. The 6.778% median paired gain was below the retention threshold and within
observed variation. Large-payload medians were A 3.350/3.340/3.404 ms versus B
3.315/3.153/4.055 ms, including one 19.125% regression. The change was reverted.

#### `TCP_NODELAY`

The sequential median moved from 1.623 ms to 1.580 ms, a 2.65% improvement below the retention
threshold. The small batch regressed from 5.515 ms to 5.589 ms (1.34%), and the large batch
regressed from 3.384 ms to 3.981 ms (17.64%). The change was reverted; the sequential benchmark was
retained to keep this tradeoff visible.

#### Encode delegate frames before locking pending requests

The targeted 12 x 64 KiB same-connection workload moved from 7.836 ms to 7.820 ms (0.20%), while
the large batch regressed from 3.230 ms to 3.416 ms (5.76%). The change was reverted; the delegate
serialization benchmark was retained.

#### Supervise all cells in one task

The 32-cell trivial workload moved from 16.050 ms to 16.100 ms (0.312% slower), and the sequential
case moved from 1.652 ms to 1.705 ms (3.208% slower). The change was reverted; the trivial-cell
benchmark was retained.

### Validation

```sh
bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed \
  --test_arg=--test \
  //codex-rs/code-mode-host:stdio-throughput-bench \
  //codex-rs/code-mode-host:websocket-throughput-bench
bazel test --cache_test_results=no --test_output=errors \
  //codex-rs/code-mode-host:code-mode-host-unit-tests \
  //codex-rs/code-mode-host:code-mode-host-stdio-test \
  //codex-rs/code-mode-host:code-mode-host-websocket-test
cd codex-rs
just test -p codex-code-mode-host
just fix -p codex-code-mode-host
just fmt
```

Both optimized benchmark smoke targets passed, covering both stdio cases and all six WebSocket
cases. The uncached Bazel unit, stdio integration, and WebSocket integration targets all passed.
The Cargo test command was attempted once and was externally blocked because the `v8` 150.4.0
build requested an unpublished Intel macOS `rusty_v8` archive whose GitHub URL returned HTTP 404.
The scoped `just fix` command reached the same external archive failure and was not retried.
Workspace formatting passed. `just bazel-lock-update` had already passed with no changes to
`Cargo.lock` or `MODULE.bazel.lock`.

## 2026-07-30: prompt-image preparation

Scope: `codex-utils-image` attachment loading, caching, and data-URL generation.

Primary benchmark:

```sh
cd codex-rs
cargo bench -p codex-utils-image --bench prompt_images -- \
  --sample-count 20 --sample-size 1
```

Targeted cache-hit benchmarks used larger sample sizes:

```sh
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 500 small_png_screenshot_repeated_load
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 100 large_jpeg_photo_repeated_load
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 100 --sample-size 1000 tiny_png
```

All results below are release-benchmark medians from the same development machine. Ranges contain
repeated runs, not confidence intervals.

### Current baseline

Use these post-change numbers as the starting point for the next experiment. Compare against the
table produced by the same command and sampling configuration.

Primary benchmark, `--sample-count 20 --sample-size 1`:

| Benchmark | Current median |
| --- | ---: |
| `small_png_screenshot_fresh_attachment` | 5.47-5.54 ms |
| `large_png_screenshot_fresh_attachment` | 163.5-163.6 ms |
| `large_jpeg_photo_fresh_attachment` | 408.9-414.9 ms |
| `one_megabyte_data_url` | 583.7-584.2 us |
| `small_png_screenshot_repeated_attachment` | 42.8-43.6 us |

Targeted cache-hit benchmarks:

| Benchmark | Sampling | Current median |
| --- | --- | ---: |
| `small_png_screenshot_repeated_load` | 50 samples x 500 iterations | 28.8-30.3 us |
| `large_jpeg_photo_repeated_load` | 50 samples x 100 iterations | 2.17-2.22 ms |
| `tiny_png_repeated_load` | 100 samples x 1,000 iterations | 279-297 ns |

### Retained wins

| Change | Workload | Before | After | Approx. result |
| --- | --- | ---: | ---: | ---: |
| Encode base64 directly into one pre-sized `String` | 1 MiB data URL | 705-716 us | 582-584 us | 18% faster |
| Replace SHA-1 image cache keys with BLAKE3 | Repeated 51 KiB PNG load | 99-106 us | 28-29 us | 72% faster |
| Use Rayon-backed BLAKE3 for inputs at least 1 MiB | Repeated 4.4 MiB JPEG load | 2.68-2.83 ms | 2.03 ms | 24-28% faster |
| Try the uncontended Tokio mutex path before `block_in_place` | Repeated tiny cached load | 479 ns | 416-418 ns | 13% faster |
| Allocate the error-reporting `PathBuf` only after a cache miss | Repeated tiny cached load | 325 ns | 279 ns | 14% faster |
| Combined retained changes | Repeated screenshot attachment | 111-112 us | 43 us | 61% faster |

Cold-cache JPEG decode/resize remained about 410-420 ms. These changes intentionally target cache
hits and data-URL construction; do not attribute noisy cold-path movement to them.

### Rejected or corrected experiments

#### Borrow RGBA bytes during PNG/WebP encoding

The candidate used `DynamicImage::as_rgba8()` and borrowed existing bytes when possible instead of
always calling `to_rgba8()`. Large PNG attachment medians overlapped the unchanged implementation
and varied substantially between runs (roughly 165-210 ms). There was no repeatable improvement, so
the change was reverted.

Do not retry this without an isolated encode-only benchmark or evidence that the resized image
still owns an avoidable RGBA copy.

#### Use parallel BLAKE3 for every input

Parallel hashing improved the 4.4 MiB fixture, but regressed the 51 KiB fixture from about 28-29 us
to 145-147 us because Rayon scheduling dominated small inputs. The retained implementation hashes
sequentially below 1 MiB and uses `update_rayon` at or above that threshold.

Re-evaluate the threshold only with a size sweep; do not remove it based only on a large-file case.

#### Benchmark repeated attachments without a Tokio runtime

The original benchmark ran `BlockingLruCache` outside a runtime, where the cache intentionally
becomes a no-op. The apparent repeated-attachment result of roughly 5-6 ms was another full image
load, not a cache hit. The benchmark now enters a Tokio runtime before warming and measuring.

Do not use the pre-runtime numbers as a baseline.

### Validation

```sh
cd codex-rs
just test -p codex-utils-cache -p codex-utils-image
just fix -p codex-utils-cache -p codex-utils-image
just fmt
```

## 2026-07-30: model-visible attachment and JSON accounting

Scope: prompt audio validation and duration estimation, original-detail image dimension accounting,
large JSON byte accounting, and the shared content-digest implementation used by attachment caches.

Benchmarks:

```sh
cd codex-rs
cargo bench -q -p codex-core --bench latency_paths -- \
  --sample-count 20 --sample-size 1
cargo bench -q -p codex-core --bench latency_paths -- \
  --sample-count 50 --sample-size 20 json
cargo bench -q -p codex-core --bench latency_paths -- \
  --sample-count 50 --sample-size 100 8000
cargo bench -q -p codex-utils-cache --bench digests -- \
  --sample-count 30 --sample-size 20
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 20 --sample-size 1 dimensions
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 30 --sample-size 20 sha1_cache_key
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 100 base64_dimensions
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 500 sha1_cache_hit
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 500 blake3_cache_hit
```

All ranges below contain medians from two warmed release-benchmark runs on the same machine. The
paired legacy functions used for before measurements were removed after comparison; the permanent
benchmarks retain the current baselines.

### Retained wins

| Change | Workload | Before | After | Approx. result |
| --- | --- | ---: | ---: | ---: |
| Validate base64 audio in a streaming pass and preserve its payload | Prepare 5 MiB PCM WAV data URL | 11.68-12.83 ms | 4.89-4.94 ms | 58-62% faster |
| Validate base64 audio in a streaming pass and preserve its payload | Prepare one-second PCM WAV | 11.71-11.75 us | 7.47-7.48 us | 36% faster |
| Read PCM WAV duration from a decoded header prefix | Estimate tokens for 5 MiB PCM WAV | 16.36-16.40 ms | 0.49-0.50 us | about 33,000x faster |
| Cache small audio estimates with BLAKE3 | Repeated one-second PCM WAV estimate | 16.71-16.73 us | 3.81-3.82 us | 77% faster |
| Count serialized JSON bytes through a writer | Measure 1 MiB JSON value | 729-738 us | 562-563 us | 23-24% faster |
| Read original-detail image dimensions from a base64 prefix | 2,560x1,440 PNG, first estimate | 11.93-12.04 ms | 1.59-1.68 us | over 7,100x faster |
| Read original-detail image dimensions from a base64 prefix | 3,264x2,448 JPEG, first estimate | 94.46-94.61 ms | 11.58-11.81 us | about 8,000x faster |
| Cache tiny original-detail image estimates with BLAKE3 | Repeated 1x1 PNG estimate | 292-293 ns | 198 ns | 32% faster |
| Bypass the image cache above 16 KiB | Repeated 1,536x864 PNG estimate | 106.0-106.7 us | 1.59-1.69 us | 63-67x faster |
| Replace SHA-1 audio keys with shared BLAKE3 | 1 MiB digest | 1.619-1.620 ms | 59.5-64.6 us | 25-27x faster |
| Replace SHA-1 audio keys with shared parallel BLAKE3 | 5 MiB digest | 8.10-8.12 ms | 229-235 us | 34-35x faster |

The prior original-detail image cache still had to SHA-1 the complete data URL before every lookup.
Hash-only lower-bound medians were 308 us for the PNG fixture and 9.07 ms for the JPEG fixture;
the prefix reader took 1.59-1.68 us and 11.58-11.81 us respectively. Removing that cache therefore
also improves repeated estimates by at least 183x for the PNG and 768x for the JPEG in these fixtures.

The WAV header fast path applies to uncompressed PCM and IEEE-float WAV files whose `fmt` and
`data` chunks appear within the progressive 256 KiB prefix, starting with a 256-byte probe. Inputs up to 16 KiB retain a small
BLAKE3-keyed cache because cache hits beat reparsing at that size. Other WAV layouts and compressed
audio formats retain the duration-probe fallback, also keyed with BLAKE3. Original-detail images use
the same 16 KiB split: tiny payloads keep the faster BLAKE3 cache, while larger payloads bypass
whole-input hashing and read dimensions from the decoded header prefix.

A 256-byte-only first image probe reduced PNG latency but moved JPEG from 12.18-12.61 us to
14.63 us. Adding a 1 KiB intermediate probe retained the PNG win and brought JPEG to 11.58-11.81 us.

### Validation

```sh
cd codex-rs
just test -p codex-utils-cache -p codex-utils-image
just test -p codex-core
just test -p codex-core audio_preparation
just test -p codex-core original_detail
just test -p codex-core executed_tool_call_recorder
just test -p codex-core counts_serialized_json_bytes
cargo clippy --benches -p codex-core -p codex-utils-cache -p codex-utils-image -- -D warnings
just fix -p codex-core -p codex-utils-cache -p codex-utils-image
just fmt
```

The utility run passed all 18 tests. Focused audio, recorder, and JSON runs passed 6, 1, and 1
tests. The original-detail filter passed its four applicable tests and also selected an RMCP test
whose local `test_stdio_server` helper was absent. The full core run passed 3,055 of 3,195 tests;
its 140 failures were in unchanged areas and clustered around missing auxiliary binaries and
timing-sensitive integration tests. Benchmark Clippy, scoped autofix, and formatting passed.

## 2026-07-30: CLI startup

Scope: root help and version rendering, plus executable-path discovery during the regular CLI
bootstrap.

Benchmark:

```sh
cd codex-rs
just bench-e2e
```

All ranges below contain medians from two warmed optimized Bazel benchmark runs on the same Intel
Mac. First-build outliers were excluded from the ranges; subsequent samples and repeated runs
confirmed each retained result.

### Retained wins

| Change | Workload | Before | After | Approx. result |
| --- | --- | ---: | ---: | ---: |
| Render a sole root help flag before main-process setup | `codex --help` | 43.97-44.18 ms | 35.98-36.62 ms | 17-19% faster |
| Render a sole root version flag before main-process setup | `codex --version` | 43.77-43.90 ms | 35.38-36.01 ms | 18-19% faster |
| Resolve the current executable once and reuse it | `codex features list` | 53.81-54.33 ms | 51.75-52.95 ms | 2-5% faster |

The root display path still performs special arg0 helper dispatch, then prints through Clap's
existing renderers before `.env` loading, helper-alias creation, thread startup, and Tokio runtime
construction. Every other invocation follows the complete bootstrap. The full-bootstrap change
shares one successful `current_exe` lookup between alias creation and async-main startup.

### Validation

```sh
cd codex-rs
just test -p codex-arg0 -p codex-cli
just fix -p codex-arg0 -p codex-cli
just fmt
```

The affected crate run passed all 348 tests, including the root-flag boundary and synchronous
preflight coverage.

## 2026-07-30: `codex exec` terminal completion

Scope: the persistent `codex exec` path from the first local Responses API byte through item
handling, rollout persistence, turn completion, and process shutdown.

Benchmark:

```sh
cd codex-rs
bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed \
  //codex-rs/exec:codex-exec-bench
```

The permanent benchmark runs a real optimized `codex-exec` child against a local streaming mock.
It reuses one Codex home so measured turns include the normal persistent-session path. The
`response_to_exit` case starts the process outside the timed region and begins timing when the mock
starts its response. The broader `persistent_turn` case measures the complete child lifetime.

All ranges below contain medians from repeated warmed runs on the same Intel Mac.

### Retained win

| Change | Workload | Before | After | Approx. result |
| --- | --- | ---: | ---: | ---: |
| Process lossless terminal notifications directly, avoiding a redundant `thread/read` history reload | Response start to process exit | 285.8-308.1 ms | 230.6-288.4 ms | 3-19% faster in paired runs |
| Same change | Complete persistent turn | 520.2-560.7 ms | 509.3-519.0 ms | 0-9% faster |

The in-process app-server transport classifies item and turn completion notifications as lossless,
and terminal turn notifications carry the last agent message summary. The additional `thread/read`
therefore repeated rollout loading immediately before `thread/unsubscribe` and shutdown. Removing
that request retains direct item processing and primary-thread completion behavior while shortening
the terminal path. The complete-turn measurement includes startup noise, so the targeted
response-to-exit result is the more sensitive comparison.

### Validation

```sh
cd codex-rs
just test -p codex-exec
just fix -p codex-exec
just fmt
```

The optimized Bazel benchmark passed with final medians of 250.6 ms for `response_to_exit` and
519.0 ms for `persistent_turn`. All 126 `codex-exec` tests passed; two existing timing-sensitive
tests passed on their configured retry. Scoped Clippy autofix and formatting passed.

## 2026-07-30: API-key plugin discovery

Scope: featured-plugin cache warming during CLI startup and the synchronous `plugin/list` path for
API-key sessions.

Network baseline:

```sh
curl -sS -o /dev/null -w '%{http_code} %{time_total}\n' \
  'https://chatgpt.com/backend-api/plugins/featured?platform=codex'
```

Three calls from the same machine all returned 401 and took 107.4 ms, 312.6 ms, and 1.412 s. This
matches the API-key code path: it could not attach ChatGPT backend authentication, yet still sent
the featured-plugin request.

### Retained win

| Change | Workload | Before | After | Result |
| --- | --- | ---: | ---: | ---: |
| Return an empty featured set when auth cannot use the ChatGPT backend | API-key featured-plugin lookup | 107.4 ms-1.412 s, then HTTP 401 | No HTTP request | Eliminates the failed network wait |

The early return happens before cache lookup, so it does not populate the cache key shared with an
anonymous caller. ChatGPT-backed auth and the existing anonymous lookup retain their request paths.
A Wiremock test verifies that API-key lookup returns an empty set with zero requests.

The optimized full-turn benchmark did not isolate a wall-time improvement because startup cache
warming is asynchronous: the prior median was 519.0 ms, while two candidate runs were 467.4 ms and
524.8 ms. Response-to-exit medians also overlapped at 244.7-309.8 ms versus 250.6 ms. The retained
result is the failed-request elimination and the corresponding synchronous `plugin/list` wait, not
a claimed full-turn speedup.

### Validation

```sh
cd codex-rs
just test -p codex-core-plugins
just fix -p codex-core-plugins
just fmt
```

All 361 `codex-core-plugins` tests passed. Both post-change optimized `codex-exec` benchmark runs
passed. Scoped Clippy autofix and formatting passed.

## 2026-07-30: zsh shell snapshot capture

Scope: local zsh environment capture during persistent CLI thread startup.

Benchmark:

```sh
cd codex-rs
bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed \
  //codex-rs/exec:codex-exec-bench
```

The paired runs used the old and candidate scripts on the same warmed Intel Mac build. Each run
contained 20 complete persistent turns and 20 response-to-exit samples.

### Retained win

| Change | Workload | Before | After | Result |
| --- | --- | ---: | ---: | ---: |
| Replace zsh snapshot counting and export-filter pipelines with zsh builtins | Complete persistent turn | 547.9 ms | 442.0 ms | 19% faster |
| Same change | Response start to process exit | 249.6 ms | 247.4 ms | Flat, as expected |

The old capture script launched nine external `awk`, `sed`, `wc`, and `tr` processes while
serializing shell options, aliases, and exports. The replacement performs those operations with
zsh arrays, pattern matching, and parameter expansion. Shell startup, `.zshrc` loading, functions,
aliases, options, exports, and snapshot validation retain their existing behavior.

### Validation

```sh
cd codex-rs
just test -p codex-core shell_snapshot
just test -p codex-core
just fix -p codex-core
just fmt
```

The focused run passed all 20 shell-snapshot unit and integration tests, including tied `PATH`,
readonly export, invalid export-name, shell environment, `apply_patch`, and cleanup coverage.
The full `codex-core` run completed 3,196 tests: 3,051 passed, 142 failed, and 3 timed out.
Every shell-snapshot test passed; the failures were in process cleanup, code mode, MCP, hooks,
sandboxing, and other integration areas outside this change.

## 2026-07-31: bundled system-skill startup loading

Scope: bundled system-skill discovery and metadata loading during persistent CLI thread startup.

Benchmark:

```sh
cd codex-rs
bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed \
  //codex-rs/exec:codex-exec-bench
```

Temporary span-close diagnostics measured `session_init.plugin_skill_warmup` around the permanent
optimized benchmark. The baseline and candidate used the same generated `CODEX_HOME` fixture on
the same Intel Mac.

### Retained win

| Change | Workload | Before | After | Result |
| --- | --- | ---: | ---: | ---: |
| Parse bundled system-skill metadata from the assets already embedded in the binary | Plugin and skill warmup during thread creation | 101-337 ms | 4.9-7.4 ms | 93-98% faster |

The system skills remain installed under `CODEX_HOME/skills/.system`, and their loaded metadata
continues to contain those absolute filesystem paths. Startup now parses the embedded `SKILL.md`
and `agents/openai.yaml` contents directly, avoiding a recursive walk, canonicalization of every
skill file, and rereading files that were generated from those same embedded assets. A parity test
compares complete `SkillMetadata` objects from the embedded and disk loaders.

Complete-turn medians overlapped because process, shell, and response-side variance dominates this
benchmark: the prior range was 465.8-506.0 ms and candidate runs were 489.2 ms and 500.0 ms. The
retained claim is the instrumented startup-phase improvement. The phase measurements were stable
across four baseline and six candidate processes.

### Validation

```sh
cd codex-rs
just test -p codex-skills
just test -p codex-core-skills
just fix -p codex-skills -p codex-core-skills
just fmt
```

All 4 `codex-skills` tests and all 128 `codex-core-skills` tests passed. The optimized end-to-end
benchmark passed twice, and the instrumented candidate run passed with six startup samples.

## 2026-07-31: fresh-thread name lookup

Scope: persistent CLI thread creation for new, cleared, and forked sessions.

Benchmark:

```sh
cd codex-rs
bazel test --compilation_mode=opt --cache_test_results=no --test_output=streamed \
  //codex-rs/exec:codex-exec-bench
```

Temporary span-close diagnostics measured `session_init.thread_name_lookup`,
`app_server.thread_start.create_thread`, and the complete thread-start request around the permanent
optimized benchmark. The source-only diagnostic changes were reverted before validation.

### Retained win

| Change | Workload | Before | After | Result |
| --- | --- | ---: | ---: | ---: |
| Read the thread-name store only when resuming an existing thread ID | Warm persistent thread start | 49-51 ms | 19-20 ms | About 60% faster |
| Same change | `thread/start` thread creation | 39-41 ms | 10-11 ms | About 73% faster |

New, cleared, and forked sessions receive fresh thread IDs, so those IDs cannot have persisted
names. Their name-lookup future now resolves locally. Resumed sessions retain the store lookup and
continue to load persisted names. The warm phase measurements exclude the first sample, which also
contained one-time process startup work.

Complete-turn medians remained within the existing variance: the prior two runs were 489.2 ms and
500.0 ms, while the candidate runs were 508.5 ms and 491.3 ms. The retained claim is the measured
thread-start phase reduction.

### Validation

```sh
cd codex-rs
just test -p codex-app-server thread_start_creates_thread_and_emits_started
just test -p codex-app-server thread_fork_creates_new_thread_and_emits_started
just test -p codex-app-server paginated_thread_name_set_is_reflected_in_read_list_and_metadata_resume
just fix -p codex-core
just fmt
```

The fresh-thread, fork, and named-resume app-server contracts passed. Both optimized end-to-end
benchmark runs passed.

## 2026-08-01: TUI buffer diff and ANSI serialization

Scope: `codex-tui` custom terminal buffer diffing and ANSI serialization for 120x40 transcript
frames. Widget computation was intentionally excluded so buffer comparison and output encoding
could be measured independently.

The pre-edit baseline and candidate comparisons used an isolated release harness outside the
worktree that included `tui/src/custom_terminal.rs` directly:

```sh
cargo run --release --offline \
  --manifest-path /Users/daniel/.copilot/session-state/54b039a9-3fdb-4501-88e2-f135c9b98838/files/tui-render-bench/Cargo.toml \
  -- --bench --color never
```

Each result below is the median range from three warmed runs with the same fixture and sampling:

- unchanged and sparse frames: 50 samples x 500 iterations
- dense repaint: 30 samples x 100 iterations
- sparse update: one changed status cell at `(31, 37)`
- dense repaint: all 4,800 cells changed
- hyperlink update: one OSC 8 forced-width cell changed

The retained fixture is now checked in as `tui/benches/rendering.rs`. Use this command for future
measurements:

```sh
cd codex-rs
cargo bench -p codex-tui --bench rendering -- --color never
```

### Combined result

| Benchmark | Before | After | Result |
| --- | ---: | ---: | ---: |
| Unchanged buffer diff | 111.8-112.0 us | 104.6-105.3 us | 6-7% faster |
| Sparse buffer diff | 112.1-112.4 us | 106.0-107.5 us | 4-6% faster |
| Dense buffer diff | 241.7-247.8 us | 163.3-164.0 us | 32-34% faster |
| Sparse diff + ANSI | 112.1-112.4 us | 105.6-106.2 us | 5-6% faster |
| Hyperlink diff + ANSI | 112.7-113.4 us | 105.5-105.9 us | 6-7% faster |
| Dense diff + ANSI | 356.6-362.4 us | 243.4-244.4 us | 32-33% faster |

### Retained wins

| Change | Workload | Before | After | Result |
| --- | --- | ---: | ---: | ---: |
| Skip forced-width repair collection, sorting, and deduplication unless a styled wide cell actually shrank | Dense buffer diff | 241.7-247.8 us | 206.8-208.1 us | 14-16% faster |
| Borrow changed cells in draw commands instead of cloning each `Cell` | Dense buffer diff | 206.8-208.1 us | 163.3-163.6 us | 21-22% faster |
| Write visible text and OSC 8 delimiters directly instead of formatting a `Print` command per cell | Dense diff + ANSI | 295.4-295.6 us | 243.4-244.4 us | 17-18% faster |

The forced-width fallback is still used when a styled wide cell shrinks, preserving the trailing
cell repair needed by OSC 8 hyperlinks and wide glyphs. Equal-width hyperlinks use the fast path.
The borrowed commands remain valid through serialization because they reference the current frame
buffer, which is not reset until drawing completes.

### Current baseline

The permanent workspace benchmark uses Cargo's `bench` profile, so its absolute timings should not
be compared to the isolated release harness above. Three warmed post-change runs produced:

| Benchmark | Current median |
| --- | ---: |
| `buffer_diff_unchanged` | 129.8-130.2 us |
| `buffer_diff_sparse_update` | 129.7-130.0 us |
| `buffer_diff_dense_repaint` | 174.8-176.4 us |
| `ansi_sparse_update` | 130.1-130.8 us |
| `ansi_hyperlink_update` | 130.3-130.5 us |
| `ansi_dense_repaint` | 253.2-255.2 us |

### Rejected experiments

#### Compare unchanged row slices before suffix scans

Checking full row equality to skip trailing-clear scans regressed unchanged and sparse medians from
106.2-106.8 us to 117.3-117.8 us. `Cell` equality cost more than the scans it replaced, so the
change was reverted.

#### Return before emitting reset codes for an empty draw-command iterator

The isolated empty-iterator microbenchmark became faster and emitted no bytes, but complete
unchanged-frame medians remained 106.4-107.3 us and dense medians moved from 243.5-245.6 us to
247.5-248.3 us. With no end-to-end latency win and a possible dense regression, the change was
reverted.

### Validation

```sh
just test -p codex-tui custom_terminal::tests
just test -p codex-tui terminal_hyperlinks::tests
cargo bench -p codex-tui --bench rendering -- --test
just bazel-lock-update
just fix -p codex-tui
just fmt
```

## 2026-07-30: model-visible attachment and JSON accounting

Scope: prompt audio validation and duration estimation, original-detail image dimension accounting,
large JSON byte accounting, and the shared content-digest implementation used by attachment caches.

Benchmarks:

```sh
cd codex-rs
cargo bench -q -p codex-core --bench latency_paths -- \
  --sample-count 20 --sample-size 1
cargo bench -q -p codex-core --bench latency_paths -- \
  --sample-count 50 --sample-size 20 json
cargo bench -q -p codex-core --bench latency_paths -- \
  --sample-count 50 --sample-size 100 8000
cargo bench -q -p codex-utils-cache --bench digests -- \
  --sample-count 30 --sample-size 20
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 20 --sample-size 1 dimensions
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 30 --sample-size 20 sha1_cache_key
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 100 base64_dimensions
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 500 sha1_cache_hit
cargo bench -q -p codex-utils-image --bench prompt_images -- \
  --sample-count 50 --sample-size 500 blake3_cache_hit
```

All ranges below contain medians from two warmed release-benchmark runs on the same machine. The
paired legacy functions used for before measurements were removed after comparison; the permanent
benchmarks retain the current baselines.

### Retained wins

| Change | Workload | Before | After | Approx. result |
| --- | --- | ---: | ---: | ---: |
| Validate base64 audio in a streaming pass and preserve its payload | Prepare 5 MiB PCM WAV data URL | 11.68-12.83 ms | 4.89-4.94 ms | 58-62% faster |
| Validate base64 audio in a streaming pass and preserve its payload | Prepare one-second PCM WAV | 11.71-11.75 us | 7.47-7.48 us | 36% faster |
| Read PCM WAV duration from a decoded header prefix | Estimate tokens for 5 MiB PCM WAV | 16.36-16.40 ms | 0.49-0.50 us | about 33,000x faster |
| Cache small audio estimates with BLAKE3 | Repeated one-second PCM WAV estimate | 16.71-16.73 us | 3.81-3.82 us | 77% faster |
| Count serialized JSON bytes through a writer | Measure 1 MiB JSON value | 729-738 us | 562-563 us | 23-24% faster |
| Read original-detail image dimensions from a base64 prefix | 2,560x1,440 PNG, first estimate | 11.93-12.04 ms | 1.59-1.68 us | over 7,100x faster |
| Read original-detail image dimensions from a base64 prefix | 3,264x2,448 JPEG, first estimate | 94.46-94.61 ms | 11.58-11.81 us | about 8,000x faster |
| Cache tiny original-detail image estimates with BLAKE3 | Repeated 1x1 PNG estimate | 292-293 ns | 198 ns | 32% faster |
| Bypass the image cache above 16 KiB | Repeated 1,536x864 PNG estimate | 106.0-106.7 us | 1.59-1.69 us | 63-67x faster |
| Replace SHA-1 audio keys with shared BLAKE3 | 1 MiB digest | 1.619-1.620 ms | 59.5-64.6 us | 25-27x faster |
| Replace SHA-1 audio keys with shared parallel BLAKE3 | 5 MiB digest | 8.10-8.12 ms | 229-235 us | 34-35x faster |

The prior original-detail image cache still had to SHA-1 the complete data URL before every lookup.
Hash-only lower-bound medians were 308 us for the PNG fixture and 9.07 ms for the JPEG fixture;
the prefix reader took 1.59-1.68 us and 11.58-11.81 us respectively. Removing that cache therefore
also improves repeated estimates by at least 183x for the PNG and 768x for the JPEG in these fixtures.

The WAV header fast path applies to uncompressed PCM and IEEE-float WAV files whose `fmt` and
`data` chunks appear within the progressive 256 KiB prefix, starting with a 256-byte probe. Inputs up to 16 KiB retain a small
BLAKE3-keyed cache because cache hits beat reparsing at that size. Other WAV layouts and compressed
audio formats retain the duration-probe fallback, also keyed with BLAKE3. Original-detail images use
the same 16 KiB split: tiny payloads keep the faster BLAKE3 cache, while larger payloads bypass
whole-input hashing and read dimensions from the decoded header prefix.

A 256-byte-only first image probe reduced PNG latency but moved JPEG from 12.18-12.61 us to
14.63 us. Adding a 1 KiB intermediate probe retained the PNG win and brought JPEG to 11.58-11.81 us.

### Validation

```sh
cd codex-rs
just test -p codex-utils-cache -p codex-utils-image
just test -p codex-core
just test -p codex-core audio_preparation
just test -p codex-core original_detail
just test -p codex-core executed_tool_call_recorder
just test -p codex-core counts_serialized_json_bytes
cargo clippy --benches -p codex-core -p codex-utils-cache -p codex-utils-image -- -D warnings
just fix -p codex-core -p codex-utils-cache -p codex-utils-image
just fmt
```

The utility run passed all 18 tests. Focused audio, recorder, and JSON runs passed 6, 1, and 1
tests. The original-detail filter passed its four applicable tests and also selected an RMCP test
whose local `test_stdio_server` helper was absent. The full core run passed 3,055 of 3,195 tests;
its 140 failures were in unchanged areas and clustered around missing auxiliary binaries and
timing-sensitive integration tests. Benchmark Clippy, scoped autofix, and formatting passed.

## Entry template

```md
## YYYY-MM-DD: area

Hypothesis:

Benchmark command and fixture:

Baseline:

Candidate result:

Decision: retained or reverted, with the reason.
```
