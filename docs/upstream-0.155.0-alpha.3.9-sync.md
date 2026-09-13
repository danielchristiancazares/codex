# Upstream 0.155.0-alpha.3.9 sync

The operator explicitly selected commit
`25b92859c1adcc8031959f17d52f27e579c3e094`, published by OpenAI as
`rust-v0.155.0-alpha.3.9` on its releases page on 2026-09-11.

## Recovery and replay boundaries

- Previous upstream base: `6b9826e3aa83b1a5947db50f4332cb9c65f1b340`.
- Original local and remote tip: `db75c20c2583ae4b825e6b742b4b31ae2066381c`.
- Backup: `backup/main-before-25b9285-20260912-160433`.
- Replay: `git rebase --onto refs/tags/rust-v0.155.0-alpha.3.9
  6b9826e3aa83b1a5947db50f4332cb9c65f1b340 main`.
- Publication has not been requested.

## Integration and validation

The replay retains 73 fork commits. It preserves upstream account ownership and
model-cache identity checks alongside the fork's provider-scoped credentials and
Copilot connection keys. Upstream voice, computer-activity, and resize behavior
remain alongside the fork's compact TUI.

The fork policy accepts operator-selected tagged versions, including
prereleases. The recorded upstream base is the selected tag's resolved commit.

Integration fixes retain explicit context-window overrides during unrelated
configuration refreshes and adapt captured-output metadata to the current tool
response API. Compaction fixtures use windows large enough for required context,
preserve the incoming prompt after a failed pre-turn compaction, and check the
single reduced-history retry. Legacy remote-compaction fixtures are removed;
streaming Responses and Responses Lite retain HTTP and WebSocket coverage.

TUI snapshots combine the compact fork layout with upstream voice controls,
verification prompts, model catalogs, and command-center behavior. Image-paste
fixtures use portable relative paths. Windows command-auth fixtures use batch
built-ins so a third-party `more.exe` on `PATH` cannot intercept token handling.

Python cancellation, subscriptions, async-client behavior, public-runtime
behavior, and generated-contract checks passed together (81 tests).

Both app-server schema generation passes, configuration schema generation, and
`just bazel-lock-update` passed.
The Bazel lockfile required no content change.

Validation ran on Windows. The final scoped selection covered 5,901 Rust tests:
5,900 passed together, and the updated bounded-capture replay test passed in its
targeted rerun. This includes the full TUI, model-provider, models-manager,
app-server-protocol, and shell-command package tests, plus selected core and
app-server integration tests for the changed behaviors. The complete Rust
workspace suite was not run.

Conflicted TUI snapshots were regenerated, reviewed, and accepted with truecolor
enabled. The obsolete extra-high reasoning warning snapshot stays removed.
The range diff preserves the replay boundaries described below.

Final scoped `just fix`, the CLI/core/models-manager `just clippy` check, and
`just fmt` completed. Clippy reports non-blocking argument-count and
type-complexity warnings in the existing WebSocket stream and shimmer helpers.
Tests were not rerun after the final lint/format pass.

The obsolete `0.154.4` and `0.154.0` version-only commits are omitted so the
selected upstream release supplies the workspace version. The normalization
patch for the removed legacy compaction parity test (`50a6741928`) is also
omitted.

The previous release's integration commit (`71c63a9883`) is omitted. Its
model-cache fixtures are already upstream, its Guardian tests target the
superseded implementation, and its TUI adaptations removed computer-activity
support that is present in this release. Generated fixtures are rebuilt for
the selected release.

The fork's Guardian input-bounding patch (`21a85f28f9`) is superseded by
upstream's composed-context budgeting. `ComposedContext::enforce_budget` applies
the remaining complete-request allowance, preserves required action evidence,
and evicts optional evidence with an omission notice. Both synchronous review
admission and per-step budgeting use this path.
