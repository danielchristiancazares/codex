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

## Entry template

```md
## YYYY-MM-DD: area

Hypothesis:

Benchmark command and fixture:

Baseline:

Candidate result:

Decision: retained or reverted, with the reason.
```
