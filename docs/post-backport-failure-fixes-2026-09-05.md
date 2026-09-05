# Post-backport failure repairs — 2026-09-05

## Scope

Repair the failures recorded in the [backport audit](upstream-backport-2026-09-05.md).
The maintainer requested one test pass at the end. Inspection, implementation,
Clippy, formatting, and diff review therefore precede a single combined nextest
invocation, with test retries disabled.

The repair worktree starts at `91dd5f420c` on
`fix/post-backport-failures-20260905`. Concurrent TUI visual changes in the primary
worktree are preserved separately so the recorded baseline remains reproducible.

## Causes and repairs

### Manual compaction

The task used `history_version() == 0` to identify empty history. In this fork,
that counter tracks rewrites such as compaction and rollback; normal appends do
not increment it. A conversation could therefore contain several turns while
manual compaction returned before reaching the model endpoint.

The guard now checks whether the history actually contains an item. The rewrite
counter keeps its existing cache and Guardian semantics. Existing integration
cases exercise pristine history, first compaction, repeated compaction, local
and remote requests, lifecycle events, and error reporting.

### App-server fixtures

The affected lineage, steering, empty-input, and goal tests used HTTP-only model
fixtures against the fork's WebSocket-only production transport.

- Reuse the existing WebSocket-to-SSE fixture adapter for recorded HTTP response
  scripts, including gated streaming scripts.
- Read per-request compatibility metadata from the actual WebSocket create
  frame's `client_metadata` fields. This also preserves steering updates on a
  reused connection.
- Cancel the adapter before dropping its HTTP response script.
- Register request-count notifications before checking the count so an arrival
  between the check and the await cannot be lost.

Production transport and authentication policy are unchanged.

### Terminal and status fixtures

- The ANSI capture backend now enables color output exactly as the existing
  VT100 backend does. `NO_COLOR` on the test host can no longer remove the color
  bytes that cursor-repair assertions are intended to inspect.
- Model-picker viewport coverage explicitly runs both standard and full-screen
  scrollback strategies. Gap accounting is asserted according to the selected
  strategy rather than the terminal hosting the test runner.
- MCP booting coverage expects its existing visible status indicator.
- Hook-completion coverage expects the current terminal-management status text.

### Snapshot expectations

Thirty-five candidates from the recorded failing run were reviewed before
validation. They update stale picker/status appearances, default-model labels,
and normalized workspace paths. Cursor snapshots retain their colored ANSI
expectations; the capture fixture was corrected instead.

The final run refreshed six further snapshots reached after the first assertion
in multi-case tests: Bedrock reasoning controls, two hook layouts, two constrained
OSS controls, and narrow enterprise-credit status. All six were reviewed; the
total accepted snapshot update count is 41. Normal behavioral assertions remained
enabled, and `INSTA_FORCE_PASS` was unset.

## Validation

Exactly one final test invocation completed:

- **4,464 tests run: 4,463 passed, 1 failed**, with 125 process-leak reports among
  the passing tests; 5,241 tests were outside the selected filter.
- All **69 unique failing case identities** in the two preceding failure logs
  were present and passed in this run, including cases repaired in the earlier
  backport follow-up.
- All **4,434 TUI tests** and **10 selected app-server tests** passed.
- **19 of 20 selected core integration tests** passed. The additional failure is
  `suite::compact::manual_compact_records_durable_and_local_token_usage` at
  `core/tests/suite/compact.rs:1066`: the second observed token-count event still
  reports zero where the test expects a nonzero local context estimate.

That additional assertion is recorded as a remaining issue. Its expectation was
preserved, and the maintainer's single-run limit was honored. The test run is
reported as failed overall rather than treating snapshot refresh as a full pass.

The nextest run ID is `22d87080-f326-4cc3-bda1-15ac4b3c9e51`. Its JUnit record and
the comparison against the prior failures are retained with the task recovery
artifacts.

The combined run covers all TUI tests, manual-compaction integration cases, and
the affected app-server metadata, goal, and turn-start fixtures. Snapshot refresh
uses `INSTA_UPDATE=always`; accepted snapshot content is reviewed after the run.
No intermediate or follow-up test runs were performed. `just fix` for core,
app-server, and TUI and `just fmt` completed before this run; automatic lint edits
outside these repairs were excluded. Argument-comment lint remains unavailable
because `cargo-dylint` and `dylint-link` are missing on this Windows host.

```text
just test -p codex-core -p codex-app-server -p codex-tui -E 'package(codex-tui) | (package(codex-core) & (test(manual_compact) | test(manual_local_compaction))) | (package(codex-app-server) & (test(suite::v2::client_metadata::) | test(thread_goal_keeps_original_root_until_external_objective_edit) | test(thread_goal_lifecycle_emits_analytics_and_clear_deletes_goal) | test(turn_start_with_empty_input_runs_model_request) | test(turn_start_steers_active_turn_and_returns_active_turn_id)))' --retries 0 --test-threads 8 --status-level fail
```

Windows validation uses MSVC 14.51.36231 and Windows SDK 10.0.28000.0, with the
repository's existing paired V8 archive and bindings. Linux and macOS execution
remain unverified on this host.
