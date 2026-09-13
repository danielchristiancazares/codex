# Stable release sync: 0.154.0

The operator selected `6b9826e3aa83b1a5947db50f4332cb9c65f1b340`, the published
stable OpenAI tag `rust-v0.154.0`, on 2026-09-09. The previous recorded base was
`5e3f0ee94b0719ab3d0d05cffaa75163e87668f6`. Its development branch contains 83
commits absent from this release; the release contains two commits absent from
that development snapshot.

The sync used explicit `git rebase --onto` to replay the 65 fork commits.
`git rerere` was enabled with automatic staging disabled, and conflict
resolutions were reviewed before staging.

## Integration boundaries

- Keep compact TUI presentation, inline history, provider/account switching,
  captured output, runtime-owned waits, context accounting, and compaction fixes.
- Adapt composer layout, status rendering, event handling, and compaction effort
  lookup to the release APIs. Omit patches to absent upstream voice controls,
  completion timestamps, user-verification UI, and specialized computer activity.
- Preserve the current activity when unified exec recreates the status row.
  Update keymap action counts, separators, and reasoning-heading snapshots for
  the release UI.
- Retain the model catalog identity interfaces from upstream `f31bd3adff` and
  `f046cf35df`, with their regression coverage and fixtures. The fork's Copilot
  cache isolation depends on these interfaces.
- Apply the complete Guardian prompt's 10,000-token bound directly at the
  release prompt builder. Preserve first/latest user anchors, selected protected
  messages, authorization, and the proposed action; refuse prompts whose
  required evidence cannot fit. Keep the release's section registry.
- Regenerate stable/experimental app-server schemas and the configuration
  schema, and refresh the Bazel lock after dependency integration.

## Recovery and local work

The original local and fetched remote tip was
`177d09dfe549adb9fa903134f017fc1447365ef3`.

- Branch backup: `backup/main-before-6b9826e-20260909-213527`.
- Working-change backup: `backup/wip-before-6b9826e-20260909-213527`, pointing to
  stash commit `815abc8bc4479dbe68927678fe3b09a1ab8484e3`.
- The original uncommitted changes set workspace versions from `0.154.4` to
  `0.153.4` in `codex-rs/Cargo.toml` and `codex-rs/Cargo.lock`.

After the rebase, the operator requested version `0.154.0` instead. Preserve
the earlier version edits in the working-change backup and apply the requested
version to the rebased workspace.

Publication requires authorization. If this sync is later published, use the
recorded remote tip above as the explicit force-with-lease expectation after
checking for intervening remote work.

## Validation

The model manager, model provider, and Guardian context suite passed all 187
tests. The core model-cache integration cases and both new assembled Guardian
prompt boundary tests also passed, along with 303 app-server protocol cases and
110 core Guardian cases. Stable/experimental app-server schema and
configuration schema generation succeeded; the Bazel lock refresh succeeded
without changing `MODULE.bazel.lock`.

A broader 5,709-case selection was stopped after 5,582 results: 5,508 successful
cases (including 124 reported subprocess leaks), 64 failures, and 10 timeouts.
App-server SSE fixtures repeatedly hit the fork's existing WebSocket-only
transport restriction. Other failures included compaction, provider-auth refresh,
request parity with generated captured-output IDs, and TUI snapshots. This was
not a passing complete integration run.

Comparison against the saved pre-rebase tip reproduced five TUI failures:
recap tool-set expectations, queued model selection, startup version snapshot
normalization, and two Windows working-directory snapshots. The other four
comparison cases passed on the old tip; the sync fixes the status restoration
regression and updates the three release separator snapshots. The initial
comparison build exhausted memory; retrying with two build jobs succeeded.

With terminal color support enabled, the color-dependent snapshot cases passed.
After the status fix and release reasoning-heading snapshot update, all 30
focused status/unified-exec cases passed. The five reproduced baseline TUI
failures remain outside this sync.

Scoped `just fix` completed successfully with existing test-support `expect`
and TUI type-complexity warnings. `just fmt` completed successfully; unrelated
inline-snapshot indentation churn was excluded. Tests preceded this final pass.
