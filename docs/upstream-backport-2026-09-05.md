# Upstream backports — 2026-09-05

## Scope and recovery

- Maintainer request: backport the changes still missing from the fork, then take
  over the existing partial application.
- Destination: `main` in `D:/codex`; recorded `origin/main` is
  `9d3629fba1b520feb84ce2c2df3f9aaa71a77d7e`.
- Source: `upstream/main`, fetched successfully and pinned at
  `ddf04ad26789d040f9ef6a96736f76602e35a6cc`.
- Range: the 11 source commits after the shared base
  `3b2d9a69e62745d4e1ebfda84cfc6134c529b7c4`.
- Takeover HEAD: `3c206f8a1ffd309df84c61a094bfe9e0aa674262`, with the first
  six backports committed and #42900 partially applied to 14 working paths.
- Recovery branch:
  `backup/main-before-backport-takeover-20260905-3c206f8a1f`.
- The takeover's working files, index, merge metadata, binary diff, and ref tips
  are preserved under `.git/recovery/backport-takeover-20260905-3c206f8a1f/`.
- Existing stashes and other worktrees remain intact. Each backport preserves
  source authorship and records its source with `Backport-of`.
- Final validation and formatting use the isolated worktree
  `D:/codex-backport-20260905-validation`. Concurrent visual-review edits in
  `tui/src/chatwidget/tests.rs` and `tests/visual_review.rs` are excluded from
  the backport follow-up and preserved in the original worktree.
- This sequence preserves the shared base and published fork history. Push and
  release actions are outside this request.

## Source ledger

| Source | PR | Behavior | Fork commit |
| --- | --- | --- | --- |
| `a97cf1b72e` | #42879 | Astra model-picker visibility | `4ee26bdd6a` |
| `574a36ff99` | #42883 | Exec-server RPC attempt metrics | `f69e9b6395` |
| `6ae8dcf6e1` | #42889 | Async-question editor foundations | `522bd7b1fb` |
| `218e8df926` | #42891 | Async-question TUI delivery | `531c5b260c` |
| `07f18d5ff7` | #42894 | Selectable answers | `9141954174` |
| `c126b0d8ef` | #42897 | Inline Other answers | `3c206f8a1f` |
| `d05e6d5f46` | #42900 | Independent task and memory identity | `94371062a1` |
| `be2684ede0` | #42903 | Question retention, history, and queue navigation | `8a3dca3cae` |
| `459a79eb85` | #42904 | Static Default-mode instructions | `38bb5022a6` |
| `12fe9f822c` | #42931 | Bundled Astra documentation guidance | `1a40f63e1a` |
| `ddf04ad267` | #42933 | Wait for Guardian turn analytics before shutdown | `c90da032f8` |

## Semantic reconciliation

### Task identity and lineage

Task start establishes an independent root when an inherited or recovered root
has not already been selected. Detached memory requests receive a fresh UUID for
both turn and root identity, shared by their header and client metadata; their
session and thread identity remain omitted.

The fork retains its lineage fences at mailbox injection, reserved task start,
and accepted steering. Conflicting or missing incoming ancestry suppresses root
attribution without replacing the selected identity. Queue-only mail preserves
attribution; rejected steering leaves it unchanged. Steering uses the existing
prepared settings object, replacing the old nested optional root parameter.

The coalesced-mail regression moved beside the existing lineage tests and covers
both independent and inherited roots with queue-only and conflicting mail.
Upstream integration assertions cover automatic input, background work, manual
compaction, empty turns, goals, and detached memory requests.

### Question input and rendering

Question state survives thread input restoration and reconnects. The complete
upstream sequence includes freeform and named answers, inline Other editing,
history recall/search, draft retention, replay deduplication, explicit delivery,
queue navigation, and unavailable-thread handling.

The fork's preview retains its eight-row budget, per-item bounds, rejected-steer
priority, hidden-item disclosure, and existing queue styling. The new
`pending_input_preview/questions.rs` adapter lets question navigation own the edit
hint while sharing the existing renderer. Changes in the large preview and
composer modules are confined to the existing integration points; the new
adapter lives separately. History responses continue synchronizing composer
popups, including the question composer.

The existing `AnsweredQuestion` fragment implementation caps framing at 512 UTF-8
bytes and flattens line breaks. Existing answer and input-envelope limits remain
on the delivery path.

The follow-up passes the available width into the fork's history-search footer
renderer and explicitly renders the blocking freeform editor before the new
paste-flush test types into it, preserving the fork's visibility admission rule.
Fourteen reviewed snapshots reconcile question, keymap, and model-picker output
with the fork's actual rendering.

### Other changes

- The Astra picker keeps the fork's compact labels and current-selection marker.
- Telemetry counts client RPC attempts and preserves the source's Statsig exclusion.
- Default-mode instructions use static text; the unused template dependency is removed.
- The bundled OpenAI Docs update replaces the older migration reference with the
  Astra guide and updates its local links. The source contents are preserved.
- The Guardian fixture waits for matching turn analytics before server shutdown.

## Validation

The isolated final feature selection passed **382/382 tests**. It covers question
editing/delivery, keymaps, pending-input previews, paste flushing, model picking,
core lineage and steering, turn metadata, automatic/background turns, detached
memory requests, bounded question framing, RPC attempt metrics, OTLP export, and
representative Guardian all-tool/computer-use approval cases.

Broader validation retains explicit gaps. The previous integration's outstanding
failures are recorded in [the September 4 audit](upstream-backport-2026-09-04.md).
Results from this session are:

| Run | Result |
| --- | --- |
| Initial focused selection | 418 run: 392 passed (1 leaky), 24 failed, 2 timed out |
| Full TUI plus config keymaps | 4,446 run: 4,402 passed (163 leaky), 44 failed |
| Isolated final feature selection | 382 run: 382 passed |

The first focused run included 13 snapshot failures corrected by the reviewed
updates. Other failures include HTTP fixtures incompatible with the fork's
WebSocket-only production transport, goal/empty-turn fixture setup, and legacy
remote-compaction expectations. Their failing setup precedes the new identity
assertions. Existing app-server mock-provider configuration, client-metadata
fixtures, and compaction task dispatch are unchanged by this batch.

The full TUI run also exposed the new covering-editor setup issue and the second
queue-shortcut snapshot; both pass in the final feature selection. The other
42 cases retain broader validation gaps, including existing style/path snapshots,
terminal cursor behavior, viewport history-gap tracking, and status expectations.
Unrelated snapshot candidates remain unaccepted. These bulk runs are recorded as
failures, including their process-leak reports.

- `git diff --check 9d3629fba1..HEAD`: passed through the source backports.
- `git range-diff 3b2d9a69e6..ddf04ad267 9d3629fba1..HEAD`: all 11 source commits
  have corresponding backports; fork adaptations are described above.
- `just bazel-lock-update`: passed; the module lockfile was already current.
- `just bazel-lock-check`: passed, with existing platforms/rules_cc version warnings.
- `just write-config-schema`: passed; the checked-in schema was already current.
- Initial compilation found an ambiguous test-macro import and the missing fork
  history-footer width argument. Both were corrected before the passing run.
- The installed `cargo-insta` uses `--manifest-path codex-rs/tui/Cargo.toml`
  instead of the documented `-p` option. Pending snapshots were reviewed and
  accepted with that manifest scope; the final additional snapshot used an exact
  extended Windows path filter.
- Final `just fix` for the nine selected crates and `just fmt`: passed in the
  isolated worktree. The retained lint adjustment initializes the question
  layout's notes height directly from its existing branches. Automatic fixes in
  eight unrelated files were excluded and preserved in a recovery patch.
  Behavior tests preceded the final lint/format pass.
- Argument-comment lint is unavailable on this Windows host. The primary
  `just argument-comment-lint -p codex-core -p codex-tui` recipe is Unix-only;
  `just argument-comment-lint-from-source -p codex-core -p codex-tui` requires
  the missing `cargo-dylint` and `dylint-link` tools.

Windows validation uses MSVC 14.51.36231 and Windows SDK 10.0.28000.0, with the
repository's paired V8 archive and sandbox bindings. Cargo incremental builds and
dev/test debug symbols are disabled to bound build storage. Linux and macOS
execution remain unverified in this Windows-host session.

### Final feature command

Run from the isolated repository root with the Windows build environment above:

```text
just test -p codex-tui -p codex-core -p codex-app-server -p codex-exec-server -p codex-otel -p codex-models-manager -p codex-memories-write -p codex-context-fragments -p codex-config -E '(package(codex-tui) & (test(question) | test(keymap) | test(model_selection_popup) | test(flush_buffered_typing) | test(pending_input_preview))) | (package(codex-core) & (test(root_turn_lineage_tests) | test(turn_input::tests) | test(turn_metadata) | test(idle_response_items) | test(ephemeral_system))) | package(codex-models-manager) | (package(codex-exec-server) & test(client_metrics)) | test(otlp_http_exporter_sends_metrics) | test(memories_startup_phase1_uses_live_thread) | test(answered_question) | (package(codex-config) & test(keymap)) | test(guardian_v2_low_risk_actions_skip_subsequent_reviews) | test(guardian_v2_computer_use_only_scopes_classification_and_fast_reviews)' --retries 0 --status-level fail
```

The full TUI/config run used the same package selection with
`-E 'package(codex-tui) | (package(codex-config) & test(keymap))'`.
