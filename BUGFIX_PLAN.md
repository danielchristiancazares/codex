# Current Codex bug-fix plan

## Baseline and scope

This plan was initially revalidated against
`fdffb238aa184a3773576117c3e20906d6c59b57` on 2026-08-27. The implementation
was then reapplied onto the clean fork overlay
`7ee06b83cf22acd23069de4d5d9f8bd9e31d57b4`, whose parent is upstream
`5f49aba876922d6f2f55caa153bbb0ed1b46feba`. It covers reproducible correctness
and bounded-resource defects whose fixes fit existing abstractions. Line
references from the earlier audit were replaced with current symbols because
the upstream merge moved several paths substantially.

The clean-base integration branch for this work is
`integrate/upstream-cleanbase-20260827`.

Two collaborator-owned TUI artifacts predate this branch and stay outside this
work:

- `codex-rs/tui/src/tui/history_batch_retry_tests.rs`
- `codex-rs/tui/src/tui/.screen_size_tests.rs.pending-snap`

## Corrected audit decisions

### Intentional WebSocket recovery policy

Unlimited WebSocket recovery is an intentional product policy. Retryable
WebSocket stream failures remain on WebSocket transport without a retry-count
ceiling. The separate feature-gated branch in `core/src/responses_retry.rs`
applies its network-wait behavior to sampling connection failures. TU-08 and the
`426 Upgrade Required` WebSocket path preserve that policy as well.

Non-WebSocket stream handling honors `stream_max_retries`. The former T2 finding
and all retry-cap changes derived from it are retired.

### Provider request replay policy

HTTP transport retries are an explicit provider configuration. A safe change to
their replay semantics needs a provider contract defining the role of
`x-client-request-id` and deduplication after ambiguous acceptance. The former
T3 proposal belongs in that provider-contract project.

### Budget semantics

Rollout and goal budgets transition after accepted usage is accounted. Current
documentation describes crossing the budget, and existing tests preserve
in-flight usage after the transition. Pre-dispatch reservation and wire
`max_output_tokens` enforcement are product changes. The former T10 and T19
findings are retired from this bug-fix branch.

### Completion delivery semantics

Status tools provide a recovery path when asynchronous completion delivery is
missed. T8 will bound every automatic completion projection. Result identifiers,
paged reads, and metadata-only status APIs from the former T9 proposal are a
separate API feature.

### Larger request-pipeline redesigns

The former T11-T14 proposals combine request preparation, compaction admission,
summary acceptance, and turn-persistence ordering. The current tree also has a
token-budget compaction path that creates a fresh context window without model
summarization. These changes need focused designs and dedicated integration
stages:

- exact prepared-request accounting, including tools and output schemas;
- pending-turn admission before transport;
- local and remote compaction fitting;
- a measured mid-turn compaction progress guard.

The existing TODO in `session/turn.rs` remains the tracking point for pending
input admission. This branch avoids changing persistence or compaction
acceptance semantics.

### Feature-scoped retry proposals

Memory schema repair and realtime event replay require their own service
contracts. Memory jobs intentionally have a persisted retry budget. Realtime
event-ID deduplication depends on server support. The former T16 and T17 designs
move to those feature projects.

## Retained findings

| ID | Severity | Current defect | Fix boundary |
| --- | --- | --- | --- |
| T1 | High | Deterministic `response.incomplete` outcomes use the generic retryable stream error and discard reported usage. | Add a terminal incomplete error carrying reason, response ID, and usage; record accepted usage once. |
| T4 | High | Malformed JSON and malformed model-visible events can be logged and skipped. | Fail the logical response with a bounded protocol diagnostic while retaining completed items. |
| T5 | High/Medium | `response.completed` with `usage: null` leaves context accounting stale. | Recompute context usage through the existing estimator; include all history when no model item or usage baseline exists. |
| T6 | High | `tool_search.limit`, returned definitions, persisted search output, and source listings have oversized paths. | Clamp count and serialized bytes at schema, handler, history, and description boundaries. |
| T7 | High | Images and encrypted function output bypass the shared truncation budget; tool images also receive independent patch budgets. | Charge every modality, enforce image count/byte limits, and share one patch budget per tool output. |
| T8 | High | Successful automatic subagent completions bypass the existing 1,000-token envelope budget. | Bound successful and error completion projections in both automatic delivery formats. |
| T15 | Medium | WebSocket prewarm returns success after EOF without a terminal completion. | Require `response.completed` and reset ambiguous WebSocket state on EOF. |
| T18 | Medium | `ImageGenerationCall.result` base64 is estimated as text. | Replace base64 bytes with the existing resized-image estimate. |
| T20 | Medium/Low | Context-window hints, including the legacy MCP thread hint, have no aggregate byte ceiling. | Bound the fully rendered hint payload to 4 KiB with a UTF-8-safe marker. |
| T21 | Medium/Low | Project instruction limits are applied after a complete file read. | Consume `read_file_stream` only through the retained prefix and drop the stream at the boundary. |
| T22 | Low | The memory summary is fully read before its model-visible truncation. | Reject files above 64 KiB from opened-handle metadata and use a one-byte growth sentinel. |

## Branch implementation status

All retained findings in the table are reapplied on
`integrate/upstream-cleanbase-20260827`, above the dedicated personal-fork
baseline commit. The bug-fix layer also restores the fork's intentional
unbounded WebSocket recovery and removes the upstream HTTP fallback path that
conflicted with the existing WebSocket retry regressions.

For review, the code falls into three coherent slices:

1. T1/T4/T5/T15 plus WebSocket policy preservation: terminal response and
   prewarm semantics.
2. T6/T7/T8/T18/T20: model-visible bounds and accounting.
3. T21/T22: bounded instruction-file I/O and its dependency lock refresh.

## Implementation details

### T1 — terminal incomplete responses

Files:

- `codex-rs/codex-api/src/error.rs`
- `codex-rs/codex-api/src/api_bridge.rs`
- `codex-rs/codex-api/src/sse/responses.rs`
- `codex-rs/protocol/src/error.rs`
- `codex-rs/core/src/session/turn.rs`

Add an internal terminal error carrying:

- `reason`;
- recoverable response ID;
- recoverable `TokenUsage`.

The parser extracts usage with the completed-response usage shape. Core records
the exact usage through the existing accounting path before returning the
terminal error. Ordinary WebSocket connection recovery remains untouched.

Regression: an incomplete response followed by a success trap produces one
logical request, retains completed output, records exact usage, and emits the
incomplete diagnostic.

### T4 — terminal protocol errors for malformed model-visible events

Use a dedicated internal `ResponseProtocol` error. Its diagnostic contains the
event kind and parse failure. The API-layer error retains at most 2 KiB of raw
payload for local diagnosis while its display string and telemetry omit that
payload. Known events that require a field return this error when the field or
typed payload is absent. Unknown future event names and explicitly ignored event
names keep their current forward-compatible behavior.

Both SSE and WebSocket readers stop the logical response at the first protocol
error. Completed output items already delivered to core remain in history.

### T5 — missing-usage recovery

When `response.completed` contains no usage, call the existing full-history
recomputation after completed output items have been recorded. Change
`items_after_last_model_generated_item` to start at index zero when history has
neither a model-generated item nor a recorded usage baseline.

This stage restores context-window accounting. Exact prepared-request estimates
for tools and final output schemas stay with the larger request-pipeline project
listed above.

### T6 — bounded deferred tool discovery

Constants:

```text
TOOL_SEARCH_MAX_RESULTS       = 32
TOOL_SEARCH_MAX_OUTPUT_BYTES  = 32 KiB
TOOL_SEARCH_SOURCE_MAX_BYTES  = 16 KiB
```

Actions:

1. Add optional integer `minimum` and `maximum` fields to `JsonSchema`.
2. Advertise `limit` as an integer from 1 through 32.
3. Clamp the handler independently of schema validation.
4. Retain whole function definitions in stable result order while both the leaf
   count and serialized output fit.
5. Apply the same helper when `ToolSearchOutput` enters history.
6. Bound source names, descriptions, separators, and the truncation marker
   inside one 16 KiB section.

The bounded call/output pair remains in history so deferred tools stay callable
on stateless later requests.

### T7 — one multimodal function-output budget

Constants:

```text
MAX_FUNCTION_OUTPUT_IMAGES              = 4
MAX_FUNCTION_OUTPUT_IMAGE_ENCODED_BYTES  = 8 MiB
MAX_FUNCTION_OUTPUT_IMAGE_PATCHES        = 10,000
```

Text, audio, images, and encrypted content debit the same `TruncationPolicy`.
Retention stays at whole-item boundaries for non-text content. One bounded text
marker reports omitted modality counts when budget remains. A zero budget yields
an empty content vector.

Image preparation uses one patch remainder for each function/custom-tool output
vector. User-message images keep their current per-image behavior.

### T8 — bounded automatic subagent completion

Apply the existing 1,000-token envelope budget to successful completion text as
well as errors. The rendered envelope reserves 100 tokens for metadata. Both
`InterAgentCompletionMessage` and `SubagentNotification` receive the bounded
status before entering model-visible history.

### T15 — completed prewarm

Track whether prewarm receives `ResponseEvent::Completed`. EOF before that event
resets the session's WebSocket request/receiver state and returns a terminal
stream error. A later prewarm starts with a fresh handshake.

### T18 — image-generation estimation

For a nonempty `ImageGenerationCall.result`, subtract the base64 payload length
from serialized JSON bytes and add `RESIZED_IMAGE_BYTES_ESTIMATE`. Empty results
retain only their wrapper cost.

### T20 — bounded context-window hints

`TokenBudgetContext::new` bounds the joined hint string to 4 KiB. The fixed
truncation marker participates in that ceiling, and truncation occurs at a UTF-8
boundary. This covers extension-provided context-window hints and the legacy MCP
`notes.thread_hint` bridge with one aggregate rule.

### T21 — streamed project instructions

For each discovered project instruction file:

1. Open `ExecutorFileSystem::read_file_stream`.
2. Reserve at most the current remaining project-doc budget.
3. Append chunks through that boundary.
4. Drop the stream immediately after a crossing chunk.
5. Preserve file order, lossy UTF-8 decoding, provenance, warnings, and the
   shared multi-environment budget.
6. Preserve the disappearing-file race behavior by skipping an item-level
   `NotFound` error.

### T22 — bounded memory-summary reads

Open the file, inspect metadata on that handle, and accept at most 64 KiB. Read
through `AsyncReadExt::take(64 KiB + 1)` so concurrent growth is detected. Decode
accepted bytes as UTF-8, then preserve the current trim and 2,500-token model
truncation.

Enabling Tokio `io-util` requires `just bazel-lock-update` in the same change.

## Regression matrix

| ID | Coverage |
| --- | --- |
| T1 | SSE parser and core integration trap test for terminal incomplete usage. |
| T4 | Table-driven parser cases plus one core partial-output trap test. |
| T5 | History-only tail estimate and completed-without-usage integration coverage. |
| T6 | Schema bounds, handler clamp, serialized output bound, and resumed-history bound. |
| T7 | Mixed modality truncation plus shared tool-image patch budget. |
| T8 | Successful completion and V1 notification bounds. |
| T15 | Prewarm EOF resets state and returns an error. |
| T18 | Image result estimate stays stable as base64 length grows. |
| T20 | Multibyte hint remains within 4 KiB and ends with the marker. |
| T21 | Instrumented stream proves the crossing chunk is the final poll. |
| T22 | Sparse 64 KiB-plus-one file is rejected before content-sized allocation. |

## Pre-clean-base validation record

The focused regressions in the matrix passed, including terminal incomplete and
malformed-event handling, missing-usage accounting, WebSocket recovery and
prewarm, bounded tool discovery, multimodal truncation, subagent completion,
image estimation, context hints, streamed project instructions, and bounded
memory-summary reads.

The following affected crate suites passed:

```text
just test -p codex-protocol
just test -p codex-api
just test -p codex-tools
just test -p codex-utils-output-truncation
just test -p codex-memories-extension
just test -p codex-response-debug-context
just test -p codex-otel
```

`just test -p codex-core` ran 3,328 tests. Its affected regressions passed when
run through focused filters. The crate-wide run also exposed existing fixture
and single-crate test-binary setup failures outside this branch's paths.

The collaborator approved the required full `just test` validation. Its cold
workspace build exhausted the 70 GiB available on the drive during linking, so
the command did not reach test execution. The generated `dev` artifacts were
removed with `cargo clean --profile dev`; source and collaborator-owned files
were unaffected.

Dependency metadata was refreshed with `just bazel-lock-update`, and
`just bazel-lock-check` passed. The terminal multi-package `just fix` pass
completed. A follow-up `just clippy ... --no-deps` completed with the existing
`ServiceTier::clone()` and destructuring warnings from the branch base. `just
fmt` and `just fmt-check` passed after all source changes.

## Clean-base revalidation

The preserved WIP applied cleanly to the personal-fork baseline
`7ee06b83cf22acd23069de4d5d9f8bd9e31d57b4`. Its 46 tracked paths have no path
overlap with the ten upstream commits added after the previous integration
point. The two collaborator-owned TUI artifacts remain untracked and outside
the bug-fix layer.

Current targeted validation passed:

- 794 tests across `codex-protocol`, `codex-api`, `codex-tools`,
  `codex-utils-output-truncation`, `codex-memories-extension`,
  `codex-response-debug-context`, `codex-otel`, and `codex-guardian-v2`;
- 125 `codex-cli` doctor tests;
- 20 focused `codex-core` regressions covering every retained finding;
- `just bazel-lock-check`.

Broader core filters reproduced four baseline fixture failures outside the WIP
paths. Three remote-aware MCP stdio search tests completed before sending their
expected model requests. One OTEL test expects quoted `service_tier` formatting
while the captured tracing event emits `service_tier=fast`. The WIP-specific
tool-search and OTEL regressions pass when run through focused filters.

The approved workspace-wide `just test` run completed in 1,684 seconds. Its
JUnit record at `codex-rs/target/nextest/local/junit.xml` contains 15,662 tests:
15,434 passed and 228 failed, including 46 timeouts. Failure review identified
two WIP integration expectations:

- Code Mode still expected five images after T7 introduced the four-image
  function-output ceiling. The integration coverage now verifies four retained
  images plus the bounded omission marker.
- The remote-environment test server implemented `fs/readFile` while T21 uses
  the existing `fs/open`, `fs/readBlock`, and `fs/close` streaming transport.
  The fixture now serves that protocol and still verifies one logical AGENTS.md
  read.

Both corrected regressions pass through focused `codex-core` filters. Review of
the remaining failures classifies them as baseline schema drift, remote/process
fixture failures, Windows platform limitations, concurrent-run timeouts, and
TUI snapshot drift outside the bug-fix paths. Command-backed provider-auth
failures and the OTEL service-tier formatting mismatch reproduce in isolation.

The run's generated scratch directories and pending snapshot outputs were
removed after exact dry-run classification. The two collaborator-owned TUI
artifacts remained untouched.
