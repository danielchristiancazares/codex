# Upstream backport audit — 2026-09-04

## Scope and recovery

- Destination: `main` in `D:/codex`.
- Initial HEAD: `deeda34ff30f08391c439fc70d555f16bd056575`.
- Recorded `origin/main`: `72a5dd866bd399065e1d2ae1d16200c1a8bca548`.
- Completed work and version 0.153.4 checkpoint: `99e6dbb05d`.
- Recovery ref: `backup/main-before-upstream-20260904-99e6dbb05d`.
- Shared base: `5f49aba876922d6f2f55caa153bbb0ed1b46feba`.
- Latest published upstream release verified: [0.153.2](https://github.com/openai/codex/releases/tag/rust-v0.153.2). The maintainer-selected fork version is 0.153.4.
- Initially pinned source: `upstream/main` at `a482e65b8643509f2217b3a34453f3c4a1968228`.
- After disk recovery, upstream refresh advanced the pinned source to `3b2d9a69e62745d4e1ebfda84cfc6134c529b7c4`, adding 11 commits to the requested range.
- Source range now contains 356 commits. Git patch equivalence initially identified #42529 as present.
- The handoff-resumption refresh fetched `d05e6d5f46505976ec4a195f0a3bb6d6e617851e`.
  The explicitly handed-off source pin remains `3b2d9a69e62745d4e1ebfda84cfc6134c529b7c4`;
  commits beyond that pin are outside this audit's 356-source scope.
- The next continuation refreshed upstream to `459a79eb85400af759e9220c7bafb4429ae07516`;
  the audited source pin and 356-source scope remain unchanged.
- Existing stashes and other worktrees are preserved. This task includes local commits; pushes and releases remain separate.
- Existing prepared semantic backports are available on `integrate/upstream-semantic-backports-20260903`; each reused change is checked against its upstream intent and current fork behavior.

## Integration policy

### Consolidated final-state integration

The maintainer switched the remaining reconciliation to an isolated consolidated
merge rather than continuing the source-by-source replay:

- Integration worktree: `D:\codex-consolidated-20260904`.
- Integration branch: `integrate/consolidated-upstream-20260904`.
- Fork parent: `9c992b1f37c68f6459695b4e7c3df13d62681da5`.
- Source parent remains the pinned
  `3b2d9a69e62745d4e1ebfda84cfc6134c529b7c4`, not the newer tracking tip.
- Recovery refs:
  `backup/main-before-consolidated-20260904-2106` and
  `backup/origin-main-before-consolidated-20260904-2106`.
- The original `D:\codex` worktree, its unmerged index, and entry 74's partial
  application are left untouched. An additional session-local recovery copy
  retains all ten working files and the original index.

The source ledger below records the completed chronological backports and their
evidence. Its remaining `pending` rows are preservation/reconciliation checklist
items, not instructions to replay intermediate source states. This merge starts
with all 73 completed backports and their follow-up fixes in its first-parent
history. No pending row becomes complete merely because Git resolves its text.

Source ownership is partitioned across core session/history, core tools/transport,
Guardian/extensions, TUI lifecycle, TUI chat widget, TUI composer/rendering,
MCP/plugins/hooks, server/protocol, and storage/platforms. Cargo metadata and
build/packaging reconciliation have separate owners. Shared builds, schema
generation, staging, and final validation wait for source edits to finish.

The final-state review must preserve:

- Shared provider/auth runtimes, WebSocket-only inference, prepared retry
  representation, and exact rejected-credential invalidation.
- Incremental context, complete-item bounds, pending-call token estimation,
  ownership-aware rollback/reconstruction, and selected-step attribution.
- Explicit reasoning/service-tier/permission/goal state, current-goal fences,
  idempotent execution-failure accounting, and persistence-error suspension.
- Fork TUI rendering, geometry, input bounds, navigation, status controls,
  provider selection, and regression snapshots.
- MCP cache generation and refresh ownership, cleanup hook attribution, account
  approvals, and bounded tool publication.
- Canonical package layout, platform-native helpers and permission checks,
  explicit Cargo dependency features, Rust 1.98.1, and fork version 0.153.4.

Consolidated merge status: source and merge-index reconciliation completed on the
isolated integration branch. All 250 conflicted paths have explicit resolutions.
The maintainer directed immediate final reconciliation on September 4 and stopped
further test expansion. This is an integration checkpoint, not a release-ready
claim. `main` has not moved. Pushes and releases remain separate.

The interrupted corrective run recovered outcomes for the original 221-case
failure index: 108 passed, 8 passed with process-leak reports, 50 failed, 5 timed
out, and 50 had no completed rerun result. The latter group includes one obsolete
test deliberately removed by pinned upstream. Later source and snapshot repairs
have not all been rerun. Detailed failure logs and the per-case progress index
remain in the session recovery artifacts. The final one-case Windows sandbox run
was stopped at the maintainer's direction.

Remaining limitations are recorded rather than hidden: Guardian/server fixture
sequencing, core compaction and skill fixtures, Windows process lifecycle cases,
and some rendering expectations still lack passing final evidence. No further
Clippy fix/build cycle is being launched during this checkpoint. `just fmt`
completed; the merge checkpoint includes the reviewed final-state snapshots and
excludes generated test workspaces. The original partial-backport worktree and
its recovery copy remain separate.

Consolidated validation evidence so far (not a readiness claim):

- Locked offline Cargo metadata resolves 153 workspace packages at 0.153.4.
- `just bazel-lock-update`, `just write-config-schema`,
  `just write-app-server-schema`, and
  `just write-app-server-schema --experimental` completed successfully.
- With `CODEX_REPO_ROOT` set to the isolated worktree, the Windows-compatible
  packaging subset passed 12 tests:
  `python -m unittest scripts.codex_package.test_archive scripts.codex_package.test_cargo scripts.codex_package.test_cli scripts.codex_package.test_zsh.ResolveZshBinTest.test_uses_manifest_override`.
  The full packaging suite has six pre-existing Windows-host errors from POSIX
  executable-mode fixtures. The affected implementation and test files match
  both merge parents; permission checks were not weakened.
- After initializing the installed MSVC environment with `vcvars64.bat`,
  `python -m unittest discover -s third_party\voice -p "test_*.py"` passed
  (54 tests, including 23 platform skips).
- Full final-state Rust behavior coverage and Linux/macOS validation remain
  outstanding. The initial fixed Cargo selection was
  27 packages spanning core, TUI, server/protocol, providers, MCP/hooks,
  Guardian/goals/history, rollout/storage, and native execution. Compilation
  succeeded before the latest corrective edits. The first combined run completed 11,815 cases: 11,594 passed,
  202 failed, and 19 timed out; 1,958 were skipped. The failures are undergoing
  subsystem-owned repair and are not a final behavioral pass.
- Compiler and preservation follow-up restored the bounded diff-preview module,
  read-only Guardian history-tool initialization, all three root-lineage
  ambiguity producers (mailbox injection, steering, and reserved task start),
  and the history-notes result cap. New lineage coverage distinguishes matching,
  conflicting, missing, queue-only, and rejected input. Model-picker refreshes
  now carry the active thread's provider through the existing discovery API.
- The pinned upstream deliberately removes the obsolete featured-plugin
  API-key gate test; that deletion is retained. Public featured-catalog access
  and its authentication-header filtering are unchanged from both parents.
- `just bazel-lock-check` also completed, with the existing platforms/rules_cc
  direct-dependency warnings.
- Argument-comment lint is unavailable on this host: its main recipe is
  Unix-only, and `just argument-comment-lint-from-source -p codex-core` reports
  missing `cargo-dylint`/`dylint-link`. The wrapper's six Python cases have three
  pre-existing Windows path-separator assertion failures; the merged diff in
  those files is formatting-only. `just test-github-scripts` finds no tests in
  the fork's retained `.github/scripts` layout and is not counted as a pass.

### Historical chronological policy

Carry source behavior and regression coverage in source order, retaining fork-owned provider support,
exhaustive state types, context bounds, UI work, build configuration, and packaging.
Commits retain source authorship and a `Backport-of` trailer. Reused prepared changes additionally
identify their local source with `Adapted-from`. Generated artifacts are refreshed after integration.

## Validation

- Checkpoint: staged whitespace review passed; Cargo metadata confirmed 144 workspace packages at 0.153.4.
- `just fmt-check` passed before the version-only update.
- `just bazel-lock-update` passed after the version update.
- Earlier `just bazel-lock-check` passed, with existing direct-dependency version warnings for platforms and rules_cc.
- `just test -p codex-goal-extension`: 30 passed. The first run exposed an upstream test expectation that needed the fork's flattened operation-result shape; fixed in `e7e4d9afa0`.
- `just test -p codex-protocol permissions::tests`: 37 passed.
- `just test -p codex-windows-sandbox deny_read`: 29 passed, including junction and canonical-path traversal coverage.
- `just test -p codex-state projects`: 10 passed.
- Stable and experimental `just write-app-server-schema` completed for project recency sorting.
- Focused core batch: 35/37 passed initially; both code-mode lifecycle cases required the standalone host binary. After building it, `just test -p codex-core mcp_result_processing --retries 0` passed all five lifecycle cases. Fixture prerequisite checking is committed in `8ec4dc7ea9`.
- `just test -p codex-code-mode-host parse_listen_url`: 2 passed. The initial `transport_tests` filter built the host but selected zero tests; `--no-run` is incompatible with the recipe's `--no-fail-fast`.
- `just test -p codex-history-notes-extension`: 7 passed after backend-budget adaptation, including ciphertext overhead and bounded plaintext coverage.
- Stable and experimental API exports regenerated again for authentication recovery notifications.
- Combined history-extension / TUI keymap / auth-progress rendering batch: 116 passed. Covers image propagation into inference and count/byte safety boundaries. An earlier compile encountered the in-progress history constructor merge; the completed merge passed this rerun.
- `just bazel-lock-update` passed after the history-image test dependency update; `MODULE.bazel.lock` remained unchanged.
- `just write-config-schema` passed after the first 44 backports; the schema was already current.
- `git diff --check 99e6dbb05d..HEAD` passed through source #41354.
- Focused cross-crate batch through #41354: 460/480 passed initially. The 20 failures comprised stale service-tier assertions, a missing standalone MCP test server, and app-server fixtures using HTTP against the fork's WebSocket-only runtime. Test fixtures now use the existing WebSocket helpers, preserving runtime transport restrictions and the `Fast` to `priority` wire mapping. All 28 selected corrective-rerun cases passed.
- `just test -p codex-rmcp-client -E 'binary(mcp_2026_stdio_discovery)' --retries 0`: 3 passed. An initial name filter built `test_stdio_server.exe` but selected zero tests; the binary filter ran the intended coverage.
- The tightened service-tier reset assertion passed its focused rerun (1/1); fixture corrections are committed in `e9d25caf2c`.
- `just write-config-schema` passed again through source #41380; generated schema matched the integrated source.
- The full rollout/thread-store and focused core/plugin batch passed **422/422** after four older fixtures were aligned with the fork's explicit `ReasoningMode::Standard` (`e57fd12d52`). Command: `just test -p codex-rollout -p codex-thread-store -p codex-core -p codex-core-plugins -E 'package(codex-rollout) | package(codex-thread-store) | test(rollout_compression) | test(exec_command_guidance) | test(multi_agent_mode) | test(recommended_plugins) | test(request_plugin_install)' --retries 0`.
- Stable and experimental app-server schema regeneration passed for shell-command timeouts (#41384).
- Bazel lock refresh passed unchanged for Guardian classifier UUIDs and the new `codex-guardian-context` crate; the new crate uses version 0.153.4.
- Initial validation through source 58 exhausted D: space with Windows error 112 before test execution. The maintainer cleared build artifacts; the resumed check found approximately 174 GiB free. Subsequent debug validation uses `--config build.incremental=false` to limit cache growth.
- MCP authorization/header-helper and OAuth-startup rerun passed **16/16**: `just test -p codex-rmcp-client --config build.incremental=false -E 'test(http_headers) | test(www_authenticate) | binary(streamable_http_oauth_startup)' --retries 0`. The standalone HTTP helper was rebuilt. Unix-only transport concurrency tests remain unverified on this Windows host.
- Combined core/Guardian/app-server/plugin/CLI validation through source 58 passed 429/433 initially. Two Guardian expectations included the currently reviewed call; a socket-reuse fixture emitted an early-result delta that intentionally releases its connection; and the parallel-review HTTP fixture inherited WebSocket capability. Test-only corrections preserve production transport and transcript behavior. All six selected corrective/adjacent cases passed, including parallel trunk/fork retries, lineage, stale completion handling, authenticated socket reuse and early-result coverage.
- Current ledger: **73 backported, 1 already present, 282 pending**.
- Source #41413 passed 322/322 protocol/composer tests and repeated release benchmarks. Retained large-history and Unicode gains, with operator-authorized small-turn tradeoffs, are recorded in [the history performance audit](upstream-history-performance-2026-09-04.md) and mirrored into the existing ignored `PERFORMANCE_LOG.md`.
- Full backport validation is pending. Final tests precede `just fix` and `just fmt`.
- Notification-media filtering's default/opt-in JSON-RPC fixtures use the fork's WebSocket transport. Both passed in the entry-61 batch, which also completed config schema regeneration.
- The first entry-61 batch ran 43 cases: 41 passed, including both notification-media
  integrations and 12 native MCP output-limit cases. Two older app-server realtime
  fixtures selected by the broad `function_output` filter used HTTP against the fork's
  WebSocket-only child runtime. The follow-up filter targets context-manager output tests.
- Entry-61 handoff boundary work reserves the complete serialized tool-result envelope,
  rejects oversized normalized outputs before model transmission and inference-trace
  start, and documents irreducible empty wire framing for nominal zero/tiny budgets.
  Initial builds exposed two corrected compile issues: fallible serialized-size accounting
  and a regression assertion that needed the fork's `CodexErrorDetails` accessor.
- The corrected entry-61 focused batch passed **441/441**, including all 14 native MCP
  output-limit integrations, both notification-media cases, 285 config tests, 15 history
  tests, 101 tools tests, and the new complete-item/tiny-budget checks. The terminal wait
  was interrupted during operator coordination; the completed JUnit report recorded zero
  failures and zero errors (run `e59e8e63-d6fc-461c-99f9-9918e317c932`). Independent boundary
  review found no unresolved defect after the compile fixes.
- Additional entry-61 transport/config/compaction coverage passed 44/45 initially.
  The existing exec metadata fixture requested 200 bytes while its fixed header and
  omission notice alone serialized to 218 bytes. Its producer and byte-truncation
  algorithm were unchanged by this backport. The fixture budgets now leave room for
  metadata plus retained first/last lines while still requiring truncation; production
  exec-output behavior is unchanged. The targeted corrective rerun passed **1/1**.
- Config and both stable/experimental app-server schema generation passed for entry 61.
  Decoded export review found exactly the expected stable internal `RolloutLine.json`
  metadata field; public API exports and the experimental bundle remained unchanged.
- App-server protocol validation against the regenerated artifacts passed **303/303**
  (the schema-writing test is intentionally ignored during ordinary test runs).
- Entry 62's shared Guardian collector passed **8/8** tests, including independent
  reasoning/tool-output selection. `just bazel-lock-update` passed with no module-lock
  delta. The new serde_json dependency explicitly requests `alloc`; package version
  and existing dependency features remain fork-owned. This foundation has no live
  model-context caller; final assembled-item bounds remain a documented consumer duty.
- Entry 63 initially passed **35/38** (all 30 rollout-reconstruction cases and five
  of eight nested-agent cases). Diagnostic reruns varied between six and seven
  successful nested-agent cases and identified a child finishing with mock-server
  HTTP 404. The source fixture supplied exactly two followup responses; late child
  completion notifications can require more. The corrected fixture bounds additional
  replies and matches only already-recorded spawn calls, retaining descendant-completion
  and exact context-count assertions. All **8/8** corrective cases passed. Independent
  review confirmed that `trigger_turn=false` preserves idle parents while mailbox input
  can require an additional sampling continuation in an already-active parent turn.
- Entry 64 notification-media validation passed **3/3**, covering both public
  default/opt-in model-input integrations and full started/completed output notification
  objects. Mixed/media-only structured outputs filter correctly; text, encrypted content,
  order and notification metadata are retained. Existing auth-recovery pass-through
  variants stay exhaustive and unduplicated. No API shape or dependency changes.
- Entry 65 extracted capture/retention into `session/step_capture.rs` and
  moved the new tests into a sibling retention module. Corrected an import ambiguity
  from the extraction. The first behavioral batch passed 50/60: nine older request
  expectations omitted the fork's explicit `reasoning.mode = standard`. The raw SSE
  fallback-interruption fixture needed the unit-test crate's explicit SSE opt-in,
  separate from the linked test-support crate's static; compression is disabled for
  that plain-HTTP fixture as well.
- Entry 65's semantic review found speculative captures publishing shared plugin and
  Responses tool-inventory metadata before selection. Candidate snapshots now retain
  plugin identity data, and selection publishes the exact finalized router inventory,
  including clearing prior inventory when the chosen router has none. Regression
  coverage exercises real primary/fallback captures and Lite-to-non-Lite selection.
- The revised 62-case step-settings/activation/retention batch passed 60 initially;
  the two remaining fixture issues were corrected. The final focused retention run
  passed **6/6**, including primary/fallback selection, completion/interruption and
  metadata publication. Source 66 supplies the live executor-hook consumer coverage.
- Residual for focused follow-up: outer legacy/v2 PostCompact lifecycle attribution
  remains initiating-primary after a successful current-model fallback. Installed history,
  completion and retained execution context use the selected fallback. Changing the
  outer hook/error/analytics contract requires a separate lifecycle decision and coverage.
- Entry 66 terminal hooks moved into `hook_runtime/turn_hooks.rs`.
  Interrupt uses its detached turn's retained step, preserving turn-scoped Code Mode
  cancellation, rollback cancellation/draining, and shared MCP runtime ownership.
  Stop/Interrupt input model and approval mode now come from the same selected settings
  as their MCP metadata, with initial-settings fallback only when no step exists.
- Entry-66 filtering/registration/environment/cancellation coverage passed **9/9**.
  Its inherited integration module was Unix-gated; the portable module now also runs on
  native Windows with an explicit PowerShell sleep command for its local-shell case.
  All **7/7** live integrations passed, including two model-switch regressions covering
  Stop and Interrupt and the no-step/no-prior-discovery case. Existing step-settings
  test helpers are reused within the integration suite.
- Config schema regeneration passed unchanged after the method-only hook-config
  refactor. Entry 66 introduced no dependency or public protocol shape change.
- Entry 67 adds SubagentStop to the same five bundled cleanup identities and MCP
  target allowlist. All **10/10** focused plugin/hook checks passed, including mixed
  manifest filtering and asynchronous, non-controlling SubagentStop cleanup dispatch.
- Entry 68 terminal-query handling passed **2/2** Windows-available tests: driver-backed
  replies/EOF flushing and unsupported-sequence preservation across all chunk boundaries.
  The direct PTY shell case remains Unix-only. TTY gating and all Windows sandbox
  launch inputs are preserved; dependency features remain explicit. Bazel lock refresh
  passed with no Cargo/module lockfile delta.
- `just argument-comment-lint` is Unix-only. The Windows source-wrapper fallback was attempted for protocol/TUI and exited because `cargo-dylint` and `dylint-link` are absent; its pinned-nightly prerequisites remain uninstalled. Report this check limitation explicitly.
- Entry 69 preserves standard MCP, legacy `openai/form`, and new `openai/elicitation`
  identities through capability selection, request/response metadata, public API, and TUI
  replay. New OpenAI forms retain opaque schemas and require explicit user responses;
  the full-access exemption remains limited to enabled, non-approval input forms.
- Both app-server schema modes were regenerated. Decoded bundle review found only the
  new form variant in the expected public exports and stable internal rollout schema.
  The initial 408-case batch passed **387**, including all **303** protocol tests,
  **23** MCP policy/capability cases, RMCP transports, and **40** TUI cases. The remaining
  21 cases exposed the new fixture bridge's missing WebSocket compression negotiation.
- The corrected elicitation bridge uses the existing test transport's compression
  configuration and reconstructs complete HTTP assertion inputs from per-connection
  response history, preserving warmup/socket reuse and the shared call/output validator.
  All **52/52** corrective and adjacent cases passed, including the new and legacy form
  round trips, strict-review policy, connection-specific capabilities, and TUI handling.
- Entry 72 records host receipt-to-outcome time independently for execute/wait/terminate,
  freezes it before serialization or client backpressure, preserves it through both
  transports and cell-ID remappers, and uses it for model-visible wall time and telemetry.
  Internal untimed responses remain representable; host wire conversion requires timing.
  App-server and the host must be built together for the mandatory stdio field.
- The 384-case Code Mode batch passed **380**, including every new timing integration,
  runtime/protocol/host test, and the reused MCP bridge cases. Four older cases exposed
  two HTTP-only app fixtures, live history estimation during a pending custom call, and
  an oversized metadata fixture exceeding the conservative complete-item envelope cap.
  The shared-host, timing, image-tool discovery, and analytics fixtures now use the
  common WebSocket bridge while retaining their original HTTP assertion backend.
- Entry 72 also updates both fork-only benchmark response patterns. Bazel analysis first
  exposed a stale `prost-types-0.14.3` toolchain label; it now names the already-locked
  `0.14.4` target. Cargo dependencies and lockfiles are unchanged. The default Windows
  GNU host path was incompatible with V8's Python generation dependency. Explicit MSVC
  host and target platforms passed analysis, then failed linking Bazel's own
  `rules_rust//util/process_wrapper` bootstrap with `rust-lld.exe` before benchmark
  compilation. Both benchmark targets remain unverified on this host. Attempt:
  `bazel build --lockfile_mode=error --jobs=4 --host_platform=@rules_rs//rs/platforms:x86_64-pc-windows-msvc --platforms=@rules_rs//rs/platforms:x86_64-pc-windows-msvc //codex-rs/code-mode-host:stdio-throughput-bench-bin //codex-rs/code-mode-host:websocket-throughput-bench-bin`.
- Follow-up correction separates live token-size projection from completed-prompt pair
  validation. A running custom tool can query remaining context before its outer output
  exists; its pending call stays untouched. Shared citation/media projection and saved
  output-limit finalization remain active, and actual model requests still enforce pairing.
  The estimator and its regression live in focused sibling modules; annotated source
  history remains unchanged through copy-on-write projection.
- The oversized Code Mode fixture now supplies 8,193 argument bytes, crossing the
  existing per-call 8,192-byte bound and producing trusted small truncation markers.
  It retains four nested calls, incomplete-cell, yield, and compaction assertions while
  respecting the conservative envelope cap. Aggregate metadata bounds retain their
  existing recorder/protocol coverage; production warehouse metadata policy is unchanged.
- All **15/15** corrective/adjacent cases passed, including all four initial failures
  and recorder overflow/completeness coverage. Follow-up history/output projection checks
  passed **86/86**, including the new pending-call/annotation regression, existing strict
  prompt-normalization cases, and saved-limit/model-visible-delta agreement. The HTTP
  image-fixture enum variant used only by Unix tests has a Windows dead-code expectation;
  the live WebSocket discovery case passed again without that warning.
- Entry 73 represents execution evidence as `NoSignal`, `Unavailable`, `Recovered`,
  or `Counted`, with a separate goal-scoped `Clear`/`Tracking` streak. Only current,
  accounting-enabled turns can contribute; success dominates same-turn failures, and
  each qualifying turn is consumed once. Status-only turns preserve the streak, while
  replacement/clear/Plan transitions retain their intended reset behavior.
- Execution-block persistence failures now retain the fork's continuation suspension,
  retire the accounting turn, and emit the existing accounting-error event. Expected
  goal identity stays fenced in accounting, usage writes, persisted rereads, and the
  final status compare-and-set. All **42/42** focused goal/backend/app-server cases
  passed, including stale-goal usage protection, Plan reset, duplicate-stop cleanup,
  stale-turn success, and the live three-failure blocking flow. Initial test-compilation
  issues (macro import ambiguity and a borrowed event iterator) were corrected before
  that successful run. Bazel compile data includes the new path-loaded state module.

## Source ledger

This ledger is in progress. “Pending” entries still require source review and integration or an evidenced disposition.

| Source | Change | State | Evidence / adaptation |
| --- | --- | --- | --- |
| `4761851ff35c4ebdd35eb8801e1180a0a50fef60` | Account subagent token usage toward root goals (#41183) | backported | Preserved fork continuation failure state and thread-aware test harness. |
| `efc020d224a96d24a7d8848a7744256ab5a7755e` | Instrument stdin review size checks (#41189) | backported |  |
| `8935ff19dbf30afd4c45579b39b2f7119059b0aa` | Stabilize Guardian WebSocket tests (#41191) | backported |  |
| `c29bc99949f90687171611bb845c4c289fa85dde` | Preserve restored permission profiles in TUI sessions (#41192) | backported | Prepared candidate: `0ed6bbb8e748bbbe6a9ef91017fc06114ee45fe6`. |
| `56239d7a618896a37380c87002165ae1cc214e39` | Report affected capabilities from remote plugin syncs (#41193) | backported | Patch-id matches upstream exactly. |
| `8aea62b2d857e950cb84366602af79403b8ed545` | Harden core test fixture startup assertions (#41194) | backported | Preserved fork test context while adopting startup waits. |
| `0182ff34802c068815d5e0990ee94ec2d080b4fe` | Finalize model-specific tool plans in `ToolRouter` (#41195) | backported | Used the prepared fork-compatible exhaustive ToolRouter adaptation. |
| `035295b46ee4a5962d0e01a66a888d5bf5da4de4` | Improve sandboxing, MCP errors, and cached approvals (#41196) | backported | Reused prepared matcher-backed Windows traversal, structured MCP errors, and authorization-bound scores. |
| `124e560b9357746b6a7e2bba1fd244115fd9cc5a` | Make the optional MCP startup grace configurable (#41199) | backported |  |
| `5bf0ba3dd65952f8e37abbb7b535ee88841e2793` | Let extensions process MCP tool results (#41202) | backported |  |
| `d4d2b293b446b019e4a14e26d4d16bfc6d682f8f` | Propagate executor home directories into sandbox contexts (#41204) | backported | Prepared candidate: `aa95fba756fafe61ea3583fc90160ddb691d1ced`. |
| `e931d07b88338c1898ad37b056aedf5458037de6` | Track executor MCP discovery telemetry (#41205) | backported |  |
| `7f135e1314433a53d4e348cbf7d3df6072e5971a` | Make Ultra reasoning fallback model-aware (#41206) | backported | Retained explicit ReasoningMode, service-tier enum, and fork reasoning-summary suppression; adapted additional client tests. |
| `19321435b10927001b260152c6617be208855a61` | Propagate executor OS into turn environments (#41207) | backported | Prepared candidate: `c336e07ab1122b6954c57f151ce0edb581537206`. |
| `c4c51c56e4f34729b5f6b3069974801a48bdd1a6` | Honor per-repository plugin configuration in catalog requests (#41208) | backported |  |
| `34e74fda0eb05ce918b6ce8778857807bc9b4a6a` | Align deny-read matching with executor path semantics (#41209) | backported | Preserved native symlink-root and canonical-target deny checks alongside executor-aware URI/glob matching; kept walker canonical-path reuse. |
| `f6494dc8f5969e8576a8a0945a674f2a15ac4de6` | Enable clock tools from model metadata (#41210) | backported |  |
| `bae69125df2de0ee70b0599c7dae9fc34fb97f03` | Roll over Guardian context before follow-up reviews (#41215) | backported | Preserved fork model-visible token counting and usable-context-window calculation in rollover checks. |
| `4fea5234664ebc628b1a5322761cb132eaacc9e2` | Share linked tool mention parsing in the TUI (#41218) | backported |  |
| `e325e3acd9ab64cd287a2c4d6cd7a7cebb639618` | Retry confirmed remote registration conflicts (#41219) | backported |  |
| `2d929eb7c39a612b84e0987f2af4a4c2282249e2` | Honor turn token budgets in Guardian review rollover (#41221) | backported |  |
| `f6726e898612e27c66a25f7c6b294f6269d9ed4f` | Add recency sorting to `project/list` (#41223) | backported | Preserved ThreadProjectAssignmentOutcome and regenerated stable/experimental protocol exports from the fork. |
| `dd453867fa60bc130e2fba0a12ae4f27c2a3b732` | Move Guardian review session tests to a separate file (#41226) | backported | Moved the complete fork test module; verified whitespace-normalized token equality and unchanged production. |
| `426fa8cdab4247e5623e9617d531f6917482b947` | Use compatible PowerShell for elevated Windows sandbox commands (#41227) | backported | Prepared candidate: `2056dc2e788689ea9d3d9209b953c34f79a76d95`. |
| `e9a6d731b783fa710ca0e359e88701bc0d4c28a6` | Apply app routing policy to unauthenticated plugin reads (#41230) | backported |  |
| `91c66024c7784f8de2d8c8355bcccad40ab3c44d` | Instrument the loaded plugin cache (#41231) | backported |  |
| `dc031d4bc70965247478724f84e2ba341c2813f1` | Expose the PowerShell version in environment context (#41232) | backported | Preserved per-field environment deltas; named shell-version update states and unchanged-version context regression coverage. |
| `f1bb4c168d7b7bcfab8083d8cb34996bf2332c3a` | Sanitize history notes backend errors (#41235) | backported | Exact source patch; backend errors retain fixed sanitized messages. |
| `41d3dc56a0e1de47e30a9585c1b49253c082f8f7` | Surface model provider authentication recovery progress (#41239) | backported | Kept borrowed app-server event-handling API, fork client tests and recovery state; regenerated stable/experimental exports. Existing recovery seam retained for source alignment. |
| `ec9620c231396895194329c410f3ec360b4cadef` | Add configurable gating for the sleep tool (#41243) | backported | Exact source patch; structured sleep mode retains model-driven defaults and feature overrides. |
| `18b9e7fd9e3f6670cc4f300338e44050b2c301e4` | Include thread source in realtime connection metadata (#41250) | backported | Exact source patch; bounded header-safe saved thread source flows through all realtime transports. |
| `6be2a6ca952ac9f70676ce4dd07fda27175aa9dd` | Let the history backend enforce tool output budgets (#41260) | backported | Backend plaintext budgets clamped to fork 10K-token hard cap; complete JSON preserved, oversized fallback rejected. Ciphertext overhead accepted. All 7 extension tests pass. |
| `7d6f808b97e424da80271be8cc539e8c5437a229` | Drive keymap conflict checks from the action registry (#41285) | backported | Exact source refactor; preserves action registration and validation order, adds ordered-conflict regression coverage. |
| `94311d447587411789533c47601fd8bc9d81eb48` | Forward history note images to the model (#41292) | backported | Preserved backend text cap after stripping images, fork dependency features/versions, and four-image/8 MiB aggregate image bounds with regressions. Bazel lock update passed unchanged. |
| `dc2ccc6843abb09c9d297862dc10b6bd12a3935d` | Make subagents follow the root service tier (#41308) | backported | Reused prepared ServiceTier/ResolvedStepSettings adaptation; retained rollout-budget module, model support checks, both compaction paths, and live root preference propagation. |
| `f98649cde9402d8b055154d1121248cc72163947` | Honor required reviews when reusing Guardian scores (#41309) | backported | Prepared patch applied cleanly; required-model checks precede cached score authorization, preserving fork score generation binding. |
| `a73bf25d17805b4169ba2a2dc4329a010a3bb120` | Decouple HTTP retry backoff from overload integration testing (#41313) | backported | Exact source regression-test update; paused-time backoff checks and bounded telemetry waits. |
| `31d338a1ea89cd65a48d8ac07f50bb3917009806` | Isolate required-model Guardian approval coverage (#41322) | backported | Exact source fixture correction; empty-input model switch preserves score generation and reads buffered review events safely. |
| `5eea8d0dd3f6b38b0e457d266fd7c918eb189bb6` | Review terminal input against retained permissions (#41328) | backported | Exact prepared patch; retained authority captured as TerminalPermissions, native/executor enum preserved, current denied reads and managed network checked before nonempty writes; private grants redacted. |
| `430d26b543b219049192de559987b8cf506efacf` | Classify clock tools as built-in control tools (#41331) | backported | Exact source patch; clock tools classified as built-in controls with lifecycle/count/privacy analytics coverage. |
| `7625343977154efed8c0dadba956374992a1580b` | Preserve cached MCP tools during binding capture (#41336) | backported | Preserved configurable grace, notification-driven tool refresh and connection-owned catalog snapshots; reapplied selected timeout after refreshed client acquisition. |
| `92f887ec35098c479dbe7f0d48d23f7f955055a0` | Use refreshed MCP tool caches during binding capture (#41344) | backported | Exact prepared patch; freshest catalogs preferred, expiry fallback retained, server cache opt-out honored. |
| `868c9edb0da913a5fc699a71664e65f44f6058b0` | Assign stable IDs to generated Responses input items (#41349) | backported | Exact source delta after import conflict resolution; generated IDs assigned once before retries, existing IDs preserved. |
| `39507eea5360b994d5f41f25e456d7f86733dc70` | Reject NUL bytes in reviewed terminal input (#41354) | backported | Exact prepared patch; reviewed NUL input rejected before approval and terminal delivery. |
| `1cc81ca89a0b7660bcb332da09d1c3e966cf0298` | Support compression for shared rollout lineages (#41357) | backported | Exact source patch (patch-id 4e8862168515305b4593707163df533b0ce2587c); seekable anonymous decoded snapshots preserve JSONL offsets and first-reference compatibility; shared compression remains opt-in. |
| `f4f85add41288c2059dc1a4326a598f739e47fe9` | Measure Codex home usage at app-server startup (#41360) | backported | Exact source patch; best-effort standalone metrics scan, regular-file metadata only, cancellation and incomplete-scan handling retained. |
| `8faf7252f07127ec4f46e0f308e6cf136bb57d63` | Test resuming compressed shared rollouts (#41364) | backported | Exact source integration regression; real compression worker plus paginated resume verifies frozen parent cutoff and compressed ancestor preservation. |
| `7e41be641ed1133711e4f75b2e142de7affaff7a` | Restrict async user messages to questions (#41365) | backported | Prepared candidate: `67b37a18c9e195b551c4e2c10e1528d72246ea62`. Exact prepared tool guidance change; later source revisions remain in chronological scope. |
| `5ed294d49d64f79b25ae63cd1cdaf54db7a797fd` | Match Windows shell guidance to the executor platform (#41368) | backported | Exact source patch; shell guidance follows single executor OS, with documented host fallback for mixed/legacy environments and remote request coverage. |
| `8bcac28f93f78b70d1159d97dbf11254bfb56a49` | Preload plugin recommendations during session startup (#41375) | backported | Exact source patch; concurrent plugin recommendation prewarm, shared feature gate, cache identity guard against invalidated initializers, and startup race coverage. |
| `4ee04c0aa5833ac39b1763f6ea44c7bc777c83dd` | Clarify proactive multi-agent delegation guidance (#41380) | backported | Prepared candidate: `9d17b99637e15a3572dd5430a94774bf91da82fd`. Exact prepared bounded multi-agent instruction update and world-state snapshot; later source wording fixes remain in scope. |
| `e4d0ba4e927363f695bb8d0fef187fd229700657` | Support configurable timeouts for thread shell commands (#41384) | backported | Regenerated stable and experimental schemas; preserved fork WebSocket-only transport in active-turn timeout fixture. |
| `60fc6995608e8188c0c9f8407d6cd98676efa247` | Give Guardian classifications distinct turn identities (#41385) | backported | Preserved retry-stable classifier identity and trusted root omission; Bazel lock refresh passed unchanged. |
| `4878401e8fbd205c683255b0d224a4592ff95a09` | Add shared Guardian context primitives (#41392) | backported | New focused crate uses fork version 0.153.4 and explicit pretty_assertions std feature; Bazel lock refresh passed unchanged. |
| `b836aecd4ddca2275f064920b52585d4e36c987e` | Preserve one-shot exec when unified exec is disabled (#41393) | backported | Exact source patch with named Interactive/OneShot lifetime; retained fork terminal-permission enforcement. POSIX timeout/cancellation fixtures need compatible-host validation. |
| `bb998aeb8e8aca2f707f83ce07f52adbb3cf2eb5` | Refresh runtimes for remote plugin state changes (#41396) | backported | Exact source patch; traced changed-only notifications through fork MCP/hooks/skills invalidation before materialization-only trust refresh. |
| `d9511fb7888d98f89526d4ae019dd9be2f14199e` | Refresh MCP HTTP helper headers after authorization failures (#41400) | backported | Exact prepared single-flight header refresh with epoch fencing, effective-header retry comparison, OAuth precedence, deadline and redirect checks. |
| `3ae4225b1761c135c6d3bbc1ea0cfcfc95752cdc` | Restrict cloud task credentials to trusted origins (#41403) | backported | Exact prepared trusted-origin check precedes credential loading; preserved fork route-aware cookies and disabled redirects for authenticated cloud requests. |
| `170da98842877c730a1d8ec9ee7421e54c06bb6d` | Optimize history item lookups (#41413) | backported | Focused Linear/Indexed state and Unicode mapper modules; first-ID/rollback/UTF-8 semantics preserved. 322 tests passed; repeated measured gains and accepted small-turn costs documented in performance audit. |
| `03147407e3a078c559f92f9fbad39d13541c3049` | Add app-server notification media filtering (#41416) | backported | Preserved exhaustive fork variants; extracted WebSocket default/opt-in coverage and documented notification-only filtering. Both integration cases passed with entry 61; standalone function-output filtering follows in #41427. |
| `f742dabc6f9c575ca43428a84b66fb42a7f3e4b2` | Support per-tool MCP output limits (#41421) | backported | Adapted `c374194b19507145d959291c3316dfa211b2f419`: saved per-tool limits combine with current byte/token policy and a complete-item 10K ceiling; envelope-aware truncation and terminal outbound guard preserve metadata/hooks/resume/Code Mode. Config and stable internal schema regenerated. 441 focused, 44 adjacent, 1 corrective, and 303 protocol tests passed. |
| `0918cd2c08f6e3b1f2b1db593e632a2e092c1ea6` | Add shared Guardian transcript collection (#41422) | backported | Stateless borrowed collection, explicit serde_json alloc feature, fork version and dependency features retained. All 8 tests passed; Bazel lock refresh unchanged. No live model caller yet: documented complete-item bounds after labels, separate pools, other sections and framing. |
| `f9cdc90c2c4d38cd557deb933e592f0032a5ea6e` | Preserve context baselines across nested agent forks (#41424) | backported | Adapted `e42b66ad9d36dace1f8ad882065e9787fce1f777`: restore prior settings from the fork's surviving full WorldState plus TurnContext baseline; preserve user-only rollback counting, retained-prefix recovery and shared provider runtime. 30 replay tests and 8 corrected nested-fork integrations passed. |
| `2008d27e98d7b46170d2d464b36dbf97008611b8` | Filter media from function call output notifications (#41427) | backported | Structured function-output notification filtering preserves text/encrypted content, metadata and original model history. Retained existing auth variants without duplicate arms; documented behavior and added full notification regression coverage. All 3 tests passed. |
| `0d226929622ce177b56e35d09cf39dd001721466` | Retain the last selected step context for each turn (#41429) | backported | Focused capture/retention module; exact selected Arc survives terminal transitions. Speculative captures retain plugin identities privately and publish router-owned inventory only on selection, clearing obsolete inventory. Preserved runtime/MCP/settings ownership. Step-settings fixture alignment and unit SSE opt-in corrections passed; final retention 6/6. |
| `c2abf869d539a6326a6e5a125dfdb8a5dc488ab4` | Run executor hooks for interrupted turns (#41432) | backported | Captured-step discovery and settings drive terminal cleanup; no prior-turn discovery reuse. Preserved scoped Code Mode/rollback cancellation and shared MCP runtime. Extracted terminal-hook owner, aligned request-body model/approval mode with metadata, and enabled native Windows integrations. 9 unit and 7 live cases passed; config schema unchanged. |
| `c6bf330b42ed6fcbdcc902dc06ef38306b2e02f3` | Allow bundled browser cleanup hooks on subagent stop (#41435) | backported | Same five bundled identities and node_repl.turn_ended target now admit SubagentStop. Mixed-manifest and dispatch regressions preserve async/non-controlling cleanup and hidden lifecycle summaries. All 10 focused tests passed. |
| `0ae94fdd49b05ee7faa4d984d06a68492cb32b54` | Respond to terminal queries from TTY subprocesses (#41436) | backported | Adapted `967652b80fd754bfc2bf91f859562b96f2eecf8e`: TTY-only bounded query responder, unchanged Windows sandbox inputs, explicit dependency features. Two native tests passed, including chunk-boundary/EOF preservation; Unix direct PTY case unverified. Bazel lock refresh unchanged. |
| `eec4a23cb16de16e0c8cff7c913eed943f223df7` | Support `openai/elicitation` form requests (#41447) | backported | Preserved distinct standard/legacy/new identities, opaque schema/metadata, explicit-response policy, and TUI replay decline. Stable/experimental exports regenerated; protocol and corrected WebSocket-backed integration coverage passed. |
| `a5c581e2476fd5309af0ea8065b92bdd91aaf26e` | Clarify question handling in Default collaboration mode (#41448) | backported | Preserved availability/mode gates; optional quality questions may use the tool, unanswered requests continue, required input uses concise direct text. Focused model-preset rendering test passed (1/1); prompt remains below 1K tokens. |
| `75388bf321bffeda40e41dca2061c1cd72c2f4d4` | Rename the read-only Seatbelt platform defaults policy (#41449) | backported | Byte-preserving asset rename, with include_str and Bazel compile_data updated together; old filename has no remaining source references. All 53 Windows-available sandboxing tests passed; macOS Seatbelt execution remains unverified. |
| `48e22a5fa08b03a9d8acc6a6577dd334c5319446` | Report code mode host request durations (#41452) | backported | Per-request host timing survives stdio/gRPC and generation remaps; model output and correlated telemetry agree while output bounds remain unchanged. New timing cases passed; shared WebSocket fixture and fork benchmark patterns adapted. Bazel toolchain label aligned with Cargo.lock; benchmark compilation blocked by Windows bootstrap linking. |
| `62b458c93151595cdf2b5ef5e37aa3d8b5613aeb` | Block goals after repeated execution host failures (#41454) | backported | Adapted `ed2f26c84ba41aa3fc84e47217aeb2086723f515` into named, current-turn/goal-scoped evidence and idempotent streak accounting. Preserved continuation-error suspension and all expected-goal fences; 42 targeted cases passed with WebSocket app fixture and Bazel path-loaded module data. |
| `2181224dad147a9ed37e698b66487aba54acdb65` | Support app targets in executor plugin hooks (#41456) | pending |  |
| `03861e69ef549717c0fc7045abad56321d4a082b` | Source proactive multi-agent instructions from the model catalog (#41457) | pending | Prepared candidate: `b02ccf061477bc3edd152c238b46d29f11920713`. |
| `3c062df036070ad5819a9a74160a448b414e9b92` | Source async user message descriptions from the model catalog (#41461) | pending | Prepared candidate: `0007a8b0817b4d2d11014faceca429aefcff2587`. |
| `0b45b171ca7141fd7723f16adb59cd8e7c1a74c3` | Preserve permissions when updating session metadata (#41464) | pending | Prepared candidate: `a15d9370a3ca9fdcb6e7d78e81a23faf5625b12f`. |
| `5fc7840cf6d085a7a7b3438d69a2beb934a2a5f4` | Refresh the TUI model picker from the app server (#41467) | pending |  |
| `9a0d7a7cd09226020afb12dc0d5ceb470c54a885` | Use rules_rs platforms for release binaries (#41476) | pending |  |
| `6478a751fde8884b2fdc76486fe23175a8e795d4` | Organize bundled Rust resources under asset directories (#41477) | pending |  |
| `4210c08defe92fe8828f789b6f9fda287ad3709e` | Preserve turn lineage across goal continuations (#41562) | pending |  |
| `f5636bb733c4653a6b91413fed1aaf8842374f2e` | Restore thread cwd from owned settings snapshots (#41567) | pending |  |
| `aaa7ed042eaa1f8c6c03e41673bdf0efc7851c14` | Harden diagnostic report uploads (#41569) | pending |  |
| `b8c86376a258e55efc8e5ecfbabc21c16c07d814` | Fix proactive multi-agent instruction grammar (#41570) | pending | Prepared candidate: `966d48ce0b78cbd5f9ea9bb642acdaa5989e65cc`. |
| `63d213884daea50e4f74efc192cdc44f549b67d5` | Add Vim search motions to the composer (#41586) | pending |  |
| `dde85b435b16994f956bce08e5fb796ed94c27fd` | Move Vim history tests into the history search module (#41613) | pending |  |
| `28327355b861ab6cc76b01c7248663eb1be440cf` | Update tests for default-enabled update_plan (#41630) | pending |  |
| `0a12b855a0b21068108a8a3b311d492712737e0f` | Preserve Guardian authorization across history compaction (#41660) | pending |  |
| `cefa060695594cdeebfb4306170cc27487c8a088` | Approve the first Node REPL execution without a Guardian wait (#41666) | pending |  |
| `da23e131e68b171909407f469da1b2f72d9c4f4a` | Repair cursor-style rendering on older JediTerm terminals (#41673) | pending |  |
| `88f776588f5e73467e7659c268f8358a9a2378b6` | Set working directories for environment MCP tests (#41683) | pending |  |
| `94cbbddafc1776d5e377bca1b05932c697e82238` | Support package-style MCP server names (#41700) | pending |  |
| `79b04f1ab5079d65e84e6a8006255a9c2cf9944e` | Show actionable rate-limit banners in the TUI (#41742) | pending |  |
| `b7cd519c767c8fd4bc3581d9bc92fbab37a768c1` | Mark history ingestion requests in turn metadata (#41743) | pending |  |
| `a9519cbcdd2d664530edb2469224ee03c1056799` | Make the update_plan tool opt-in (#41744) | pending |  |
| `d58d0e5841e0de08e251673db2d5af8cf3a1ad51` | Allow models to enable token budgeting by default (#41803) | pending |  |
| `e45226e771add9f87a47b6d24c1ff61d62588846` | Use the async stack budget for approval reviews (#41840) | pending |  |
| `1c1e17782aeb51a5a253997067fa887a9d593cc9` | Preserve Guardian review evidence across compaction (#41846) | pending |  |
| `305eed102d6ab5fc1228fec0737ba240eb29826b` | Preserve Guardian user answers across compaction (#41852) | pending |  |
| `9d19c7426ecc3fe0a384be57a33bcb5909715387` | Box the session startup future at its API boundary (#41853) | pending |  |
| `98a8425e223106d6e20fb31881fd693e9c56cf63` | Preserve Guardian user answers from current history (#41857) | pending |  |
| `09f4c4506874e75e25b967d51844cb699086e124` | Preserve user text when Guardian history drops oversized images (#41858) | pending |  |
| `2c8cfbf44f9cbe16f91d7630d21b501d3e2cf817` | Keep history extension tools out of Guardian reviews (#41861) | pending |  |
| `032d15cba77e28d4eb697b1f11bc395c2522d12b` | Use shared transcript collection for Guardian reviews (#41870) | pending |  |
| `9f97cb79eb15b38d24c552c56fe24e211ff9cf3a` | Preserve Guardian review evidence across compaction (#41879) | pending |  |
| `379d50be35d393631f45fde69197f3b9a592aa02` | Add pinned native voice source preparation (#41884) | pending |  |
| `fc7d34ad67d0c398b8d2745e329273ee9ecebbaa` | Add native voice dependency build recipe (#41890) | pending |  |
| `e51b54b4c03b05680d4b301bce035d92a3e7dbef` | Retain the MCP client for event streams (#41892) | pending |  |
| `32f48598a0609a882e5847f0d3e35d6d67f375bc` | Show successful TUI commands individually (#41893) | pending |  |
| `65237aeca0bf607fb2d940a1fc22cd5d4c43de07` | Fix Windows native voice dependency builds (#41894) | pending |  |
| `d60560f14e6271da8ac8f680fcdfb9245d5b9413` | Add the voice helper lifecycle foundation (#41897) | pending |  |
| `53691bad9878d6e26ba546bc5959d2d49eaecf86` | Keep MCP event subscriptions alive after task unloading (#41899) | pending |  |
| `f88ff940c0f5b20628e94abc27d04196a14c5b94` | Load bounded context after empty wake turns (#41901) | pending |  |
| `13bc770eaf0ad8548776bde59c3d6e5316406279` | Add installed voice host lifecycle support (#41902) | pending |  |
| `ae0ecd7484a2f9b5420a618e801d03c1f0aead2f` | Add a manager for MCP event streams (#41906) | pending |  |
| `2f0a5d5516c566e40b7abefea5f3c1f81fcd64bd` | Avoid scanning archived rollouts when archiving threads (#41908) | pending |  |
| `b51b07785bcd1545f63893f363ee1957949526e0` | Make permission transforms aware of executor path context (#41909) | pending |  |
| `a7913390f7bbd819d4b80fa2f6a67c15fce6cf53` | Preserve TUI drafts after app-server disconnects (#41911) | pending |  |
| `5f79a92e3936274318d2122ae3244e5edd80dd1f` | Persist response token usage in rollout history (#41912) | pending |  |
| `17e8101699c5062117d0d37f504313e8af53b043` | Preserve TUI status timing when the status row is hidden (#41913) | pending |  |
| `9a4b78579a7f672d5c71aa442bb95072915cc5cd` | Move the config schema generator into a dedicated crate (#41915) | pending |  |
| `907c34e867223d5160ec87afbf1bf5c9b6a8d57d` | Reconnect TUI app-server sessions automatically (#41916) | pending |  |
| `0a2e2bf9570c5be586ff68f305cd36ad2e2fcbb8` | Open the agents overview from an empty composer (#41917) | pending |  |
| `746798b2f752888935a137fe0328dd4cf2b7a735` | Restore agent navigation after TUI reconnects (#41918) | pending |  |
| `865bbf9a69467e2321daa2252c775b0336e9d454` | Source Guardian REPL policy from model metadata (#41919) | pending |  |
| `0344625ccf4ae0ab6472c6c1e7b4ace6af14661e` | Start fresh Vim drafts in Insert mode (#41921) | pending |  |
| `34c4f7e72dd11a5b5a1f767fe2bc5036bd0c91f5` | Allow per-call sideband endpoints for existing realtime calls (#41923) | pending |  |
| `64c9cde458b6b1c1d332577425b927f0436ab40b` | Record realtime conversation history in Core (#41924) | pending |  |
| `9127f21890265323701ea080b6f0fe0a164bcb54` | Test repository-wide Rust formatter discovery (#41925) | pending |  |
| `c4350b4ca2a5af7dced6784e8c36f2f2cdab2dd6` | Use executor path context for permission preapproval (#41928) | pending |  |
| `2c3bf4ea793aa5c590932553d242a287380e9cec` | Open the agents overview directly in the reconnect test (#41929) | pending |  |
| `4ac20a7f748a8a12cae0eb5019a26d13fdc2d456` | Increase Guardian message transcript limits (#41931) | pending |  |
| `a62ff9a5d45736328088646ca2f14706e3ff5c17` | Report configured sandbox policy consistently (#41933) | pending |  |
| `981d6b652b710991e5e1c09e12ee7a7ee1a6c0da` | Omit undersized WAV output from Code Mode (#41934) | pending |  |
| `13d75cd1c3e9bbb69c1b9d5a73c28ef5e5e807a6` | Attach failed Guardian reviews to diagnostic reports (#41936) | pending |  |
| `115ffaf8bf1eda460526605cf44d9a96d88f4371` | Limit background terminal input previews (#41937) | pending |  |
| `0e03f88a30bbce4638aeabc62557dda90612d32c` | Clarify resume guidance in exit summaries (#41938) | pending |  |
| `c5a3700dd7ee73a07b30ce1516dae51a264bb8cf` | Preserve transcript layout caches during backtrack selection (#41940) | pending |  |
| `9c7edd4bc39f0314767431e6a5960c95c81aa814` | Add Vim undo to the TUI composer (#41941) | pending | Independently reviewed: no prepared seed. Keep bounded private undo history, full editable drafts/attachments/pastes, transaction boundaries, search cancel restoration and reset semantics; update state-machine docs/schema/snapshots. Source 1 MiB accounting omits heap-backed TextElement data: tighten or explicitly assess before retaining. |
| `e39ab0c1854dbd567172fd5a79dbfff9067cb609` | Emit turn cost telemetry for ChatGPT sessions (#41944) | pending | Independently reviewed: preserve custom-provider worker, shared auth/runtime, 4096-ID dedup bound, exact microUSD formatting and all-observed-ID settlement. Auth changes clear pending work/reprobe; log status only. Adapt in-process OTLP fixture transport. |
| `c2e5692e2c923ed222c77487b3141829881757cd` | Expand extension permission regression coverage (#41946) | pending | Independently reviewed: test-only. Preserve turn-vs-session grants, current permission checks on cached skill continuation, unchanged cached bytes and fail-closed denies; >1 MiB skill replacement warns once without replacing prior prompt. Adapt PathUri roots; restricted Windows cases remain unavailable without elevated fixture support. |
| `bfa9646787cda93c8012532ea8fa44a74fc38bfc` | Add plugin reconciliation app-server API (#41949) | pending | Independently reviewed: stable v2 RPC waits through reconciliation and hook refresh. Preserve shared sync gate/generation fences, old+new capability hints, exact-account Workspace+Listed trust, and release reconciliation before exclusive config work; refresh runtimes even after failed trust. Regenerate stable/experimental exports and README. |
| `55e5158e1841bb4b0b392a462c24b4d9fc38d597` | Improve tracing for nested tool calls and exec processes (#41950) | pending | Independently reviewed: definition-level nested-call spans and live tool traceparent preferred over saved cell context. Preserve dispatch/cancellation/frame checks; exec span begins after spawn, releases request parent and finishes on every path. Add explicit OpenTelemetry dependency/lock refresh; later #42373 supersedes lifecycle implementation. |
| `633ab199cfd724aa78013c006b27a2b3d049fc3b` | Enforce marketplace source policy for curated plugins (#41953) | pending | Independently reviewed: curated local catalogs map to canonical openai/plugins Git identity with no ref; exact/host allow rules may match, invalid rules/ref restrictions/local paths fail closed. Apply to install/read/list, implicit activation and startup sync; preserve bundled/runtime exemptions and remote-installed independence. |
| `c7fced56eb3f5b0b96f4957caefc06b07ef65940` | Track TUI starts by app server mode (#41974) | pending | Independently reviewed: no prepared equivalent. Emit once per TUI launch after resolved target/OTEL setup; keep exhaustive target variants and startup draft/workload-identity/panic-recovery flow. Do not gate the per-launch metric on process-start deduplication. |
| `d038f3448f2eb9b48614f6359a3b21113d7d42ae` | Move `disable_paste_burst` under `[tui]` (#41976) | pending | Independently reviewed: preserve legacy top-level fallback and nested Option<bool>; merged nested key wins, including across layers. Retain fork Tui fields; update exhaustive literals and config schema. Composer state-machine behavior is unchanged. |
| `e017e93aceafb2fe04bed1c926e448a5fb4f913d` | Preserve raw response usage metadata (#41980) | pending | Independently reviewed: preserve arbitrary non-null usage JSON and exact amount, hardened SSE/WS protocol errors, and refactored remote-compaction attempt emission. Regenerate stable/experimental rawResponse/completed exports; keep opaque raw-event data outside model context/persistence. |
| `2b7c279735d0d096cf7b34fe98938f46792f4d4f` | Report turn trigger and source in turn analytics (#42003) | pending | Independently reviewed: read live metadata at emission so accepted steers appear; preserve fail-closed ambiguous-root state and configured-source precedence even when oversized. Analytics caps are 128 bytes; first-set trigger and request metadata remain unchanged. |
| `82099786163f3c05facf09078136679e18b64279` | Share Guardian user-message retention logic (#42031) | pending | Independently reviewed: anchor first genuine user message even over budget; consider remaining users newest-first only when they fit, render in original order. Preserve consumer-specific pools/windows/dedup/framing costs and entry-132 final assembled-item ceiling. |
| `2e5ee418ad6bef8b418ba1a809cfa53a56ae4aee` | Improve Guardian report diagnostics (#42033) | pending | Independently reviewed: keep all evidence/index/rollouts inside include_logs and the requested subtree. Root plus newest failed descendants precede newest UUIDv7 descendants (8 selected, 64 omitted IDs); dedupe paths, preserve bounded chronological failures and clearly process-wide discard accounting. Selection index precedes attachments and describes selection, not delivery. |
| `9d0eae74cd3d543c40dcdfa4e4007b43445e6543` | Include shared histories in rollout compression (#42039) | pending |  |
| `90ae0c4ef944bb80a3c725d15910289dfbb7db51` | Tag Codex home size metrics with compression state (#42043) | pending |  |
| `0ec375eb7077a417a6a544615bded3544e4799bc` | Add per-account approval settings for apps (#42047) | pending |  |
| `0e37d834d4f2fefea63da419bbc52055af33e888` | Honor explicit account selectors for Apps tool calls (#42054) | pending |  |
| `3a04482645b695085f4daf7c6310ab8592653fea` | Honor app link settings for MCP tool approvals (#42056) | pending |  |
| `28097e98ebcb5e7eaa2e14534e60337f209a8a80` | Preserve Guardian history across thread reconstruction (#42065) | pending |  |
| `25ba0ae6cd5b41a2b99d2bd032f5001fc0ddce15` | Remove selected core test cases (#42066) | pending |  |
| `3c6bb8b3ebfe3462e781047d00bd6aae11a54c83` | Detect standalone installs from the macOS CLI bundle (#42068) | pending |  |
| `82461c99903c473d6ba1be0791ae4fdd1b52b9a5` | Remove redundant test coverage (#42069) | pending |  |
| `d1414383429bb06fa48513423708bb13e44c1f15` | Detect Vite+-managed Codex installs (#42071) | pending |  |
| `2350823caa2bd3c4a6c7ef46deb390425ca7d5e1` | Unify Guardian context section collection (#42076) | pending |  |
| `67cc3c318dc8b5532db6ade4182b1dc6f3870889` | Attribute nested REPL reviews to their tool calls (#42082) | pending |  |
| `6127478086e611323e3bff40c943588606c1c571` | Centralize Guardian context composition (#42085) | pending |  |
| `f4e6cb78760af4eb75bb370f0f15bd8ca4cb1d3a` | Attribute Guardian reviews to OpenAI app tools (#42086) | pending |  |
| `ade0ccacf9182e665cf784de041c51a79e35bc31` | Record Windows MXC availability (#42094) | pending |  |
| `a17ee5705c270ef5f243bd7f22c845341768f365` | Make diagnostic report uploads resilient to slow networks (#42096) | pending |  |
| `b6ab99ed87844a357e24c281f092db2bf2b05b9f` | Prefer remote Sites over the bundled plugin (#42100) | pending |  |
| `8ea297ff604f358d0e973f1abc34743b56dfcf92` | Add a TUI setting to disable automatic recaps (#42101) | pending |  |
| `9969043b95339ec008b2f2f317f664d763fa4825` | Extract OTEL trace WebSocket into a reusable crate (#42102) | pending |  |
| `f40e08478ce01516e841437e0c9cb051df52e146` | Show recent sessions in the agent command center (#42104) | pending |  |
| `6a479e1813fc44c1ea3c85b7b4023ee3d4c21b8f` | Show full patches and terminal input in TUI history (#42107) | pending |  |
| `84aa75204ad604bcf498df180d128181c89b2f0b` | Format Python scripts across the repository (#42109) | pending |  |
| `c7c824dce4da186e5142af5d9a1587ae553efe46` | Treat bundled cleanup hooks as built-ins (#42110) | pending |  |
| `1f4c47343a1bff2d8cddc429c5d39503fb5a6c30` | Apply remote platform semantics to exec safety checks (#42113) | pending |  |
| `ef76e6ac30990071b10c8ae7f2bcaf55dc447afc` | Centralize remote plugin mutations in `PluginsManager` (#42114) | pending |  |
| `ea7e0714daeee7ebad9a0061ed44483a8c2a50e8` | Fix relative MCP server spawning on macOS (#42117) | pending |  |
| `bc39b0ed59a85c47b2e9c267cdd57bb1018bc5f2` | Refine hook activity rendering in the TUI (#42118) | pending |  |
| `91125641141fcd8a4b026931d49e352f6626ffaa` | Allow updating the approval reviewer for active turns (#42121) | pending |  |
| `0276f2ee551f2cedf03f54ddab7fbc63a7579e94` | Preserve descriptive labels on local file links (#42123) | pending |  |
| `7730edf62c16906b4b009866f6b4f9ef03d32d3d` | Ignore non-JSON files in plugin catalog test helpers (#42126) | pending |  |
| `bd89ce67cca74783d8339aeeb842e9b673c7e2ed` | Prepare MCP connections for coordinated OAuth refresh (#42128) | pending |  |
| `8436b749a410133c58ec61fe89db07496ca22033` | Bound Git root discovery for metadata enrichment (#42132) | pending |  |
| `2393b5c9208aab4233cf5e9b1c57d1a17425bef6` | Scope session MCP approvals to app account links (#42133) | pending |  |
| `a30fa3d830475ce83bd1b4e4e584e40e135436e9` | Include app link metadata in MCP approval elicitations (#42134) | pending |  |
| `c5b5fa8089954219302092c03b8efc2024d33df6` | Support thread forks from symlinked session roots (#42135) | pending |  |
| `612e6491d50ffb80ffc4330edc4024b86e51e4bf` | Prewarm shell snapshots for eligible turns (#42137) | pending |  |
| `b192442fc7f56aebbe065030561263cd67ffa44b` | Add redo support to Vim composer history (#42140) | pending |  |
| `a876a9e49415beee2c7283b6f715cee6e81d4181` | Add early rate-limit warnings for Plus and Team plans (#42142) | pending |  |
| `86b7d127428573c8088ef7ed478e59f7bb28a9db` | Add Guardian V2 analytics events (#42144) | pending |  |
| `cd8bd62c6e289b96d5f8eb0443c6d8561c2e8c54` | Resolve permission requests in the executor context (#42146) | pending |  |
| `e5769939113536eb72752660bf7d1903f799d198` | Skip Guardian reviews in Full Access (#42147) | pending |  |
| `68c9556cdf14c069e289904ed087f62205c4ba91` | Upgrade Git marketplaces from merged configuration (#42149) | pending |  |
| `6b59cefcbb35951c197a235dc94dbe700f2fbc7c` | Support remote marketplaces in the plugin CLI (#42150) | pending |  |
| `986ff1cc7ced0081ec5014b700a376333d87f869` | Expose model settings in app-server thread metadata (#42151) | pending |  |
| `8971fc25a96c5368f6e7bd8e66799a2dec476f9a` | Split tool JSON Schema code into focused modules (#42161) | pending |  |
| `12ed76b09bf311d8d9a6f25be2f9a8eb37daf879` | Record result sources in app tool analytics (#42164) | pending |  |
| `671d5d1b3531d7251d6c59709b8d29fb7927e685` | Support header injections in network requirements (#42173) | pending |  |
| `b21f2e30815a66d96c711f4739b18e7a56e522db` | Add a cacheable Bazel app-server schema bundle (#42174) | pending |  |
| `2c79ee6dacb6deccb7e19ac5acffb3e379bbe895` | Add structured asynchronous user input requests (#42178) | pending |  |
| `ddf8a67ab09cd76b8adc0969f11ee1271179aba7` | Fix punctuation in npm packaging documentation (#42188) | pending |  |
| `02f47d3fb36414d99cdf34fff553826d587d1405` | Use native spawning for bare macOS MCP commands (#42192) | pending |  |
| `5a0419edb5ad720ae31fa1adf8b3b24c8f0c52c5` | Add Vim replace mode to the TUI composer (#42194) | pending |  |
| `798833fe97c3c9db4eb38134ccc9d184c0e37b08` | Add managed worktree creation (#42196) | pending |  |
| `0b509e930e191a463746799f4b2a05c0e1bbd15f` | Refactor shared TUI input routing (#42199) | pending |  |
| `9d57be71ba33bbb2e2dfbed244287f04e01c259f` | Separate TUI preferences from server configuration (#42202) | pending |  |
| `8813bd4b00c0aed7ffe3e86d06503c075cc775f8` | Add macOS voice runtime projection (#42204) | pending |  |
| `27bf160f7909704fb7e23d508f31900d90479699` | Retry TUI reconnects while threads are closing (#42207) | pending |  |
| `8d01cd42faebd14fc39b46c98cfa0d94e353546d` | Add GNU Linux voice runtime preparation (#42208) | pending |  |
| `eb10d91e48ccbd0930427461fb392337addb1ac0` | Add Windows voice runtime preparation (#42209) | pending |  |
| `bdfd769640927a627dd48ab3fcc1ae8bc08bdd0a` | Track history notes thread hint outcomes (#42247) | pending |  |
| `a0dcfe2ada3f5bbd5059a34c0fc6fac244741a67` | Skip Guardian scoring in User approval mode (#42256) | pending |  |
| `8d32abcd017d06511b46050cff9dbba8738fc2fa` | Report the exec-server release version in environment info (#42270) | pending |  |
| `50fffd5ed367aa99491d9ec58575626fce4e9dd4` | Refresh plugin skills after out-of-process version changes (#42284) | pending |  |
| `94e5d05095fe1ae695bca6aa8bbac0ee358cff6e` | Fetch rules_rs zlib packages from Ubuntu snapshots (#42288) | pending |  |
| `389dd5645944891b65e4ca584125bbb0c852d352` | Expand Guardian history coverage across resume and rollback (#42290) | pending |  |
| `5971d42847aae04db0e3c70146e0b189fc9a6803` | Preserve verified answers across history compaction (#42293) | pending |  |
| `8e3b180d49951c3e53140710b2baad09791cc999` | Preserve retained answers across steer rollbacks (#42298) | pending |  |
| `fc953e5234f2452e393310b2be2b29a482c4d907` | Stabilize the detached exec-server session resume test (#42306) | pending |  |
| `1bc8fb16ae53512c0c7723a436e4b11be81ad4a8` | Separate Windows sandbox provisioning from ACL refresh (#42309) | pending |  |
| `eb078b4f44b0c8099d376440d63ce5bbb11675bd` | Preserve target-native cwd in permission approval requests (#42314) | pending |  |
| `f252c23b884eb1a30afec05275d75aa1361e3fb9` | Refactor exec-server startup futures (#42316) | pending |  |
| `a94a5db6295a3c5b4e3e18da58a2f6b774900d0e` | Support packaged managed Codex binary paths (#42318) | pending |  |
| `a526f54b005f0647dec041f26a67356be88b15fe` | Show live context compaction status in the TUI (#42319) | pending |  |
| `5e26f7621c1c470fe62350d61c9eb4d6c772a0da` | Make the app-server thread unload delay configurable (#42320) | pending |  |
| `637c3227b3c5327d64a73fe0b3764ade139efd1f` | Avoid executing PATH helpers before workspace trust (#42324) | pending |  |
| `e6ff7495064372deab97e7b793f5b613d1a76309` | Render completed assistant messages directly during replay (#42325) | pending |  |
| `73e94ee7a60704d4094142c543958f6a794f78e9` | Harden Windows control socket rendezvous (#42326) | pending |  |
| `0d502a423031396a8d11c096e5b9f1cb0d30b3d0` | Support durable reasoning configuration updates (#42328) | pending |  |
| `f59905647a5ca8681b87c5900036081955fb8750` | Protect Windows sandbox binaries from inherited write access (#42330) | pending |  |
| `dc0dc4f15d2a5be395b6fa3deb2aca898f8a94d5` | Package prepared runtimes with the voice host (#42332) | pending |  |
| `301a7c5e011a57111d82575de6db5e0142fdd9e6` | Add a Windows sandbox provisioning protocol (#42334) | pending |  |
| `dcfcb570b2cd0a2500b1d47a7b04a7cb1b0a0bd2` | Add an authenticated Windows sandbox provisioning client (#42337) | pending |  |
| `501931b3995983c1eb888933bd79adfd18fccc1e` | Add Windows sandbox service lifecycle scaffolding (#42341) | pending |  |
| `add870a4bf226d87e0201b4e28ee6a159a49aed1` | Harden Windows sandbox provisioning file handling (#42342) | pending |  |
| `c4ea7294b989827862eed6972d130233459161f4` | Prepare managed policy validation for Windows sandbox provisioning (#42344) | pending |  |
| `4fdf4c11131ec901a303f68e5ad8962469697bb6` | Add Windows sandbox client authentication (#42348) | pending |  |
| `7e45bdb5fd0e6cae3aeb14330deed7861bb516da` | Enable authenticated Windows sandbox provisioning (#42351) | pending |  |
| `830363bd7ca7bd6d98c7a766da0ce2684730985c` | Add experimental Windows sandbox service provisioning (#42353) | pending |  |
| `d6350e24be4d8151a090dd74811774618fe9ecd7` | Add free-form asynchronous user messages (#42354) | pending |  |
| `0227158fd58e2c7381148bea47cf3ba56453ff99` | Initialize questions in buffered replay test messages (#42356) | pending |  |
| `577a4fcd065e07fab24db2d208d825c67558b8a5` | Extend rate limit reads with usage capabilities (#42358) | pending |  |
| `10aca93f18cc7cec42ed63e4913e4f94cb531d62` | Support graceful daemon shutdown on Windows (#42364) | pending |  |
| `a2a9a434764b9ff875022c028f636a77656d1abc` | List managed worktrees for a repository (#42366) | pending |  |
| `095ac4f131e759b204fa6368dc42d2feff6eb21a` | Keep SQLite history projection moving past invalid records (#42369) | pending |  |
| `76f47103feceb43cee8b8c9a6d8a7b4bc4567e86` | Improve MCP server startup error logging (#42370) | pending |  |
| `50379197779be0e5afcbddb014a34cd1fc08af53` | Add Luna Reserve usage fallback to the TUI (#42372) | pending |  |
| `f53c91be2c1393ad5fd69b4d2a95445812598087` | Add attributed exec process lifecycle telemetry (#42373) | pending |  |
| `a14ef02e1cda08ebc0887b014b43cbf53ae98f72` | Extract PID startup into a dedicated module (#42374) | pending |  |
| `665e5f45ab91d43ec3a49487fb9be90109dd2875` | Clean up Windows sandbox resources on app uninstall (#42375) | pending |  |
| `e1d0ef995f07530d2063e0ce51259b8fce1a82bf` | Make app-server realtime sessions always available (#42377) | pending |  |
| `69cebb5d15939bf9b6c1b4647b53879beab91ba2` | Route rollout reads through the canonical JSON decoder (#42378) | pending |  |
| `0588fc941c3bd5abd0cb8cd38c51cd6e72e9eb16` | Require confirmation for safety-buffered retries (#42380) | pending |  |
| `715294448f56ce3ca6b2f62c84e9c9ef5e9aee20` | Support managed app-server lifecycle on Windows (#42381) | pending |  |
| `a28aab7587a302b09e74efdb0c46ef44aa899ab9` | Update rmcp to 3.2.0 (#42383) | pending |  |
| `312709252d36582de550a480deab20c332877edc` | Add an RMCP OAuth credential store adapter (#42384) | pending |  |
| `cff76fa96f70f9f3b63d221446fd02cfd87e6d2e` | Add experimental context management activation (#42385) | pending |  |
| `2b554fd3f96a128be52e0d64b01f6adf16cc467a` | Expose loaded thread environments in app-server responses (#42386) | pending |  |
| `e6249b52969028e0f50833bc536946763b74dbeb` | Recover deferred environments after provisioning failure (#42388) | pending |  |
| `fe140d4c8e7d47950d4d2e35ff7c58e55b744f65` | Authorize `apply_patch` in the executor path context (#42391) | pending |  |
| `91608236ea27a4f8de26a6ccfcd5e7f08a895887` | Support managed daemon updates on Windows (#42392) | pending |  |
| `9bb1ea035f262f0905ce26a372df96e486983592` | Expose the Codex version to commands and turn metadata (#42395) | pending |  |
| `e6a944ad758b41b0bdfd80079a52e3133beac910` | Extract focused TUI logic into submodules (#42397) | pending |  |
| `54a4077c8b010bd36720804adb8301c1e6883650` | Preserve restored input after resolved misalignment errors (#42399) | pending |  |
| `d4dc882998ddf7f3d2b40893ef2f77a5fdfa5715` | Discover TUI collaboration modes from the app server (#42401) | pending |  |
| `1281778e3273ab8e28c700ba84f5f12115e0ddc0` | Expose the last accepted environment ready report (#42403) | pending |  |
| `deb147116661d298c5e0408226f80342c3cc349f` | Read voice helper frames independently of pipe chunks (#42404) | pending |  |
| `b7f710273e14ef2d79d510a06385c2a67285585a` | Support the app-server daemon on Windows (#42405) | pending |  |
| `460b63e5f43cb1cc8e992ae9c3235ec22d918613` | Honor explicit plugin mentions during MCP startup (#42406) | pending |  |
| `93053c7f5ddc1c26e649e9e7ffc0d9e853c633cf` | Harden embedded composer input handling (#42408) | pending |  |
| `fdf23b4097bf19adf2286c64316da2d2a9fedae6` | Allow reviewing and continuing misalignment-paused chats (#42410) | pending |  |
| `88912c04cd1180becf77f7a74e3b10351f0779bb` | Enable coordinated MCP OAuth refresh (#42413) | pending |  |
| `b27a6321fa1a1dbb48e019d1d1296d2a13dc4261` | Expose managed application network requirements (#42417) | pending |  |
| `1d741742c5fd5d3cf5d93ea71d48a3c9af9bda41` | Add session resume to the agent command center (#42419) | pending |  |
| `38ba8cdceb536aa55af7db132d6bc830da8c0129` | Honor model requirements in Guardian computer-use scoring (#42422) | pending |  |
| `cac96cd7b1756ab42e8925d938817a2ac10ebb6e` | Discover TUI experimental features from the server (#42425) | pending |  |
| `62f553bfd02cdc1206931b99d0083fd9619d7688` | Use the shared composer in the agent command center (#42428) | pending |  |
| `498d40b29f6028dec9ef80af672ba1258980b54a` | Box the TUI resume picker future (#42432) | pending |  |
| `36984da4424cb91b6bc88c6af8d73207930ac729` | Include originator in plugin measurement analytics (#42445) | pending |  |
| `8b8ee28a9b0df8188fec8d4e3855a7b3af3ed8a2` | Acknowledge pending TUI steers by submission ID (#42451) | pending |  |
| `c9fecd3fa06af28011166207c596ad547e37abab` | Discover permission profiles from the app server (#42453) | pending |  |
| `8ff74cc9b11ac54c4c4446ef60a87b49ae657a1f` | Show live task details in the agent command center (#42455) | pending |  |
| `728cb12fe5794b0c3a8e776fb4994b1650b973a8` | Expose thread originators through the app-server API (#42458) | pending |  |
| `6d7f6dcd2285de70a3892d4f05b2a8ff44aa3350` | Register the Guardian thread context feature flag (#42529) | already-present |  |
| `0650d6d1ca451b67009b3969a82b87e76979975f` | Preserve MCP authentication challenges on tool calls (#42552) | pending | Prepared candidate: `f10f9f320cac3f1e4399a77fea152a7e7ee2cd83`. |
| `7a7c188682c3f3aae6c7efabf3acd1a9c7dfd3e6` | Preserve target-native paths in command approvals (#42577) | pending |  |
| `1d74c3ba1ee98be2025ab066dcc3fd654fe8a3b6` | Persist verified user answers in Guardian thread context (#42579) | pending |  |
| `1d6727c0b52316515bfbf5dea578374bafb4d898` | Recover Vim escape input in legacy terminals (#42584) | pending | Prepared candidate: `e7f5e08006048005c7c871456bbac70feb6be36c`. |
| `ad8ee16a5f4c7445253b57a10cb1f8489c8c3e6a` | Require Guardian review for incompatible compaction checkpoints (#42588) | pending |  |
| `ec84e692611d50d9c79c4bdad0a5785013975a72` | Harden the macOS sandbox against terminal input injection (#42590) | pending | Prepared candidate: `d1a54a5ff1379ad7161205e0716c66873c5585c1`. |
| `2387310b528e3daad579f19d1083921e5fcb1a88` | Reload user config after local plugin installation (#42593) | pending | Prepared candidate: `35b56d685ed8d89baea597179bbb3915f9ee2fc9`. |
| `f60dfe80b5c97be25e7c69b3792c0bf4bfcd3c50` | Record Windows sandbox private desktop usage (#42596) | pending |  |
| `8f31b64c7f9ef67d8f966bff5ddf9e08eafe0b4d` | Report MCP tool discovery errors in server status (#42598) | pending | Prepared candidate: `ce31ad6d72d87e20e9666996b4b5c58cbda20e87`. |
| `f84c9776dc7f6c08e677c4592827c4b8b7fdf0df` | Deprecate detached review delivery (#42602) | pending |  |
| `7eee24ef5134ee2b41ec66b185bc7703b620332a` | Expose global metrics installation in `codex-otel` (#42603) | pending | Prepared candidate: `d521a168c7a0a045c420587a10dfcb3de05238a0`. |
| `801ca0d0d1206a5886048a01262584c79bf33d60` | Support trusted headers for remote exec WebSockets (#42606) | pending |  |
| `ed391d4dd21396715b66c278e6b451897672c93c` | Add GPT-6-Astra to the bundled model catalog (#42607) | pending |  |
| `32c303c197e437cb13d444389d651fbaae02a6ed` | Condense TUI startup warnings (#42609) | pending |  |
| `1f7b99922a285f748ef323a53d421fd67ef8438d` | Add GPT-6-Astra to Amazon Bedrock catalogs (#42619) | pending |  |
| `781c183c3bf991ae938ffed7ac97d7c018c2d540` | Bound Noise handshakes by the exec server initialization timeout (#42623) | pending |  |
| `280ae8b9fcd35a54704dc2c78027b89bd51369fd` | Centralize prompt image detail modes (#42624) | pending |  |
| `d979df154cf60e13eafb5453e75b6d84f21c67bf` | Initialize the packaged GStreamer runtime in the voice host (#42631) | pending |  |
| `03467026f2426fafd3d33bbdfc78ec5f9d79f6f0` | Add an injectable attachment store to ThreadManager (#42634) | pending |  |
| `0e0f55fc4ec9308840e54ceba1f1f1dc9547380f` | Update GPT-6-Astra Fast tier speed description (#42638) | pending |  |
| `68e9c4a31a4a947d43b3af18561e5ee4c8597c5c` | Warn when saved model defaults are overridden (#42639) | pending |  |
| `0305dde9203917f7ef24bf58e94305415191685a` | Harden TUI parsing of assistant markup (#42640) | pending |  |
| `956aa3f6372ffd73a0df639beff356b3b664b858` | Restore the inline TUI after full-screen overlays (#42641) | pending |  |
| `f46671b14aa3bc37d4ee9a67c06385cb9ec8e2d3` | Render assistant file citations as local links (#42650) | pending |  |
| `eb5a00b0683f3555ce6a2ce55d0257dc95151437` | Add managed worktrees to `codex exec` (#42652) | pending |  |
| `e8b65624e0732c5e20f8604d4a81fa408760626c` | Update the stable exec-server test to Codex 0.153.1 (#42654) | pending |  |
| `a7ab2d66d781b903cb060288a89e26e8d2b9a05f` | Use a generic fallback model name in status tests (#42657) | pending |  |
| `ff2f01b0c2c4f8fdcb0b18a3e8ad0d21fe68428c` | Tailor TUI cyber refusal notices to Daybreak eligibility (#42667) | pending |  |
| `ea2046f36d5ee12d39c8e168fc3e5129301afa2b` | Cancel remote control enrollment on stdio shutdown (#42668) | pending | Prepared candidate: `8cbdf3809efe769f1cb50a53304b7d6d6daaa911`. |
| `b995d06050ee3db5c6298f4a007975da94399c71` | Preserve TUI sessions while starting replacement threads (#42671) | pending |  |
| `048a936a23b88c8653f4820e68f987de10e3c583` | Persist server-advertised experimental features from the TUI (#42674) | pending |  |
| `1b53f6a44eff890b5169bde8d3bd5b12b8766946` | Add WebRTC negotiation to the voice host (#42676) | pending |  |
| `f3f6922519fa38487c8250c2b8a670a39a2cf9ff` | Narrow async user message guidance (#42677) | pending |  |
| `8e6a44b428e31f91b21edc97904fcdf4f0931ade` | Fix the worktrees experimental feature test fixture (#42682) | pending |  |
| `d13aeb77ea0eb72a994324143e6cdffcba650963` | Allow trusted symlinks beneath CODEX_HOME on macOS (#42716) | pending |  |
| `3c837e568c24e4281bba4abdf3bc3c398f3fff13` | Gate unified exec TTY support behind a feature flag (#42718) | pending |  |
| `9d253c885cb7cc48aeb749a82e31e2070e14f73e` | Make the TUI symlink startup test Bazel-compatible (#42741) | pending |  |
| `4e48cd02da734b0dfb51726d264133e63951308d` | Honor model-provided Guardian review policies (#42744) | pending |  |
| `8e85265c39176b6bd498242a33d7b0f9b4b98303` | Handle pending network reviews after process completion (#42746) | pending |  |
| `a1294e57f12e9474088a2f6e6a9142ed3e1ddccd` | Improve automatic thread naming in the TUI (#42749) | pending |  |
| `1cd78651e24016236b7e35c4ca77b9e34f58f9f5` | Preserve response IDs for fast collaborator tool events (#42752) | pending |  |
| `97e46694e1459f758fe03c297e31f40ba1384cd4` | Stabilize the interactive tmux startup safety test (#42755) | pending |  |
| `cc4b8bdeb8446144bc6a5e3f6d31b4d5c633c6f4` | Propagate response tickets to Guardian reviews (#42758) | pending |  |
| `80d7ca34bce7497fa72a13af74695c759a48a8d4` | Retain user instructions in guardian thread context (#42762) | pending |  |
| `88f87d907a91aea5e9ea38a3e9a653bfedd71f9b` | Avoid port races in streamable HTTP tests (#42767) | pending |  |
| `99d66aa1c5f8394729a97a6eea91880fa420352b` | Preserve acceptance order in retained thread context (#42770) | pending |  |
| `c9fac4dd5a06f29b9a6525b025a92c0bc367ae40` | Avoid holding metadata permit during cold resume config load (#42773) | pending |  |
| `b3f5e45cc1de8bcb09d320f3211378db285aa201` | Add direct SigV4 transport to exec-server (#42781) | pending |  |
| `a07158c7846e5cb6684216d779598e540998f295` | Keep TUI prompt history tied to local settings (#42791) | pending |  |
| `47b0f7d540e9abf932e9b518ab306e389744998e` | Extract the note input view into its own module (#42792) | pending |  |
| `80ab0ffafe092f0738b5a3f513aabfd2cab66848` | Add data-use disclosures to the user report dialog (#42798) | pending |  |
| `89a4eec6dafce21486c5a56e6599095e7517c4b1` | Keep the Windows sandbox command runner hidden (#42801) | pending |  |
| `0ae02915bd1bfdd8207631f65aa31987368c7c8b` | Add request-scoped Guardian approval decisions (#42807) | pending |  |
| `86b1b359cf89b65c136eda8c84e5ee544a6c2cf2` | Enable staging login issuer overrides in packaged builds (#42811) | pending |  |
| `5533375287179260650fff35479c3f973b18fb9a` | Support custom report event titles (#42814) | pending |  |
| `3fde89f628281b9b049376fb0cec4d1577bfeaec` | Route Guardian approvals independently of async scoring (#42819) | pending |  |
| `387bc6ba597fb38f73d8b1af73138c279e440c3c` | Report managed filesystem policy in `codex doctor` (#42821) | pending |  |
| `0f64d7080864d22610731935cdc203bd2b3c5d93` | Expose managed WebMCP policy through the app server (#42823) | pending |  |
| `de7874067fe8cb8f4846dd4d8b848965ce79070f` | Refine user input guidance for GPT-6 (#42824) | pending |  |
| `87628df77ab1a2622d1193ad835df02ced565bf2` | Preserve root authorization context in Guardian reviews (#42832) | pending |  |
| `f1aac1e885f676a1129f2da0c46a3dba86392fc6` | Preserve SystemRoot for Windows sandbox wrapper setup (#42833) | pending |  |
| `a482e65b8643509f2217b3a34453f3c4a1968228` | Preserve Windows managed deny reads in the sandbox CLI (#42835) | pending |  |
| `83b62a02fab5c0fc797cbc9896c332148f1fd9d0` | Make GPT-6-Astra user input guidance conditional (#42836) | pending |  |
| `773f0b081de689b0d54f2809e7b17bfdb4c9f341` | Preserve executor paths in Guardian approval reviews (#42838) | pending |  |
| `60888d08685f3caa8ad4979518924d373d7477cf` | Add a native Windows MXC sandbox adapter (#42841) | pending |  |
| `147137c1f4118175d3f8db92f2f33faf9c14f6d8` | Add Astra sparkle effects to the TUI composer (#42842) | pending |  |
| `9c4253ffc1b954337bf2f494aadc55e9cd132a48` | Retain user instructions in Guardian context (#42844) | pending |  |
| `7a8092a4479ad401c36b58580f1a3aeafe9ce890` | Preserve Markdown formatting when copying TUI responses (#42847) | pending |  |
| `8e4b7d31dede18adba90479cab2bbf75c258ad40` | Use jemalloc for Linux musl binaries (#42850) | pending |  |
| `4636819a352bfd4271c669c568a92f1bccfcd7e0` | Harden Guardian reviews after context compaction (#42852) | pending |  |
| `3921a30d6b11218883e5bb81a48082095cf79b7c` | Persist Daybreak preferences in thread metadata (#42854) | pending |  |
| `d2d5b70241fb448044c1c088a977cc720d70443a` | Preserve precedence across feature requirement aliases (#42863) | pending |  |
| `3b2d9a69e62745d4e1ebfda84cfc6134c529b7c4` | Avoid redundant filesystem sandbox path resolution (#42870) | pending |  |
