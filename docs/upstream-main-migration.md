# Fork migration onto upstream/main

This migration starts from OpenAI's `b4373e53ab79df7baadc6805dea54a060b820307`
and ports selected behavior from the personal fork's
`a925b5b3c3843a08c85e38a47de8dd7f21d8fbdb`. Their shared ancestor is
`3b2d9a69e62745d4e1ebfda84cfc6134c529b7c4`. The source fork is preserved by
`backup/main-before-rebase-20260907-a925b5b3c3`.

The maintained product branch is `main`, with the fork commits rebased onto
OpenAI's fetched `upstream/main`. `origin/main` publishes the product; the original
fork history remains available through the dated backup ref above.
See [fork-workflow.md](fork-workflow.md) for the current working agreement,
repository-local Git setup, and rebase procedure.

## Ownership and integration boundaries

| Behavior                                                               | Owner                                                            | Upstream integration points                                  |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------ |
| Copilot credentials and device login                                   | `login/src/copilot/`, `cli/src/copilot_login.rs`                 | Auth manager, CLI command dispatch                           |
| Copilot identity, endpoints, models, quotas, and request normalization | `model-provider/src/copilot/`                                    | Provider registry, transport auth hooks, model-manager trait |
| Provider discovery and switching                                       | `tui/src/app/provider_switch.rs`, `chatwidget/provider_popup.rs` | Provider-scoped `model/list`, app events, thread start/fork  |
| Compact execution, inspection, MCP, and edit history                   | `exec_cell/`, `exec_command/exploration.rs`, `history_cell/`     | Existing tool lifecycle handlers                             |
| Responsive selection and composer presentation                         | `bottom_pane/` layout and presentation modules                   | Existing selection and input state machines                  |
| Inline history preservation                                            | `transcript_reflow.rs`, `app/resize_reflow.rs`, `tui/`           | Terminal drawing and stream consolidation                    |
| Deferred raw-output changes                                            | `chatwidget/history_render_mode.rs`                              | App input and stream completion                              |
| Transport and notification efficiency                                  | Existing transport, telemetry, event mapper, thread-store owners | Small changes at shared event boundaries                     |

The provider remains an implementation of the current upstream provider API.
Authentication is shared by its intended callers; request preparation occurs
before WebSocket serialization. Provider discovery does not switch a thread.
Switching preserves saved history and respects active work, remote workspaces,
background processes, and unsaved history.
Resume reconciliation only applies when a local session changes providers.
Remote sessions keep server-owned configuration, and ordinary resumes retain
upstream's support for model IDs absent from the picker catalog.

UI grouping changes presentation, not execution or approval classification.
The shell-command display parser explicitly preserves unsupported shell statements.
Full transcript output remains available behind compact activity rows.

Inline terminal scrollback is not rewritten on resize. Owned-buffer replay
retains its separate policy. Raw-output changes during an inline stream take
effect after consolidation so one response is not rendered with mixed policies.

## Upstream behavior retained

The migration preserves current upstream model selection and collaboration APIs,
selected permission profiles, managed worktrees, external-writer recovery,
completion timestamps, live voice support, and grouped computer actions.
The final fork versions of central files could not be copied wholesale because
they predate some of these features.

Upstream already contains the archive filename filtering optimization. The
fork's older raw-WebSocket-payload retention fix is also unnecessary on this base.
Performance changes requiring fork-only realtime-history effects or context-token
projection are excluded. The broad token-audit overlay, separate reasoning-mode
API, dependency/default-feature churn, and platform packaging changes are outside
this migration. They should be evaluated as separate product changes if needed.

## Keeping future rebases manageable

Keep product work on `main` and use `upstream/main` as the upstream reference.
Before rebasing, preserve local work and verify the intended upstream commit.
The initial commit series isolates storage, first-token timing, request metadata,
delta identifiers, notification names, listener/subscriber reuse, telemetry, and
release LTO policy. Copilot authentication and transport stay together so their
credential, retry, request-shaping, and environment boundaries move as one change.
The provider catalog API and display-only shell parser are separate commits.

The initial TUI migration is one integration commit: composer, history, status,
viewport behavior, and the shared styles affect overlapping snapshot expectations.
Keeping those snapshots with the complete rendering change avoids introducing
temporary UI states solely to split the migration. For ongoing development,
keep commits grouped by focused behavior:

1. Copilot authentication and provider support.
2. Provider catalog API and TUI switching.
3. Display-only shell parsing and compact command, MCP, and edit history.
4. Responsive selection widgets and composer presentation.
5. Status, startup, and shortcut presentation.
6. Terminal history and stream-rendering policy.
7. Independent performance changes, with their measurements.

Keep generated protocol/configuration artifacts with the change that requires
them. Avoid mixing formatting, dependency updates, or unrelated cleanups into a
behavioral commit. Prefer additive modules; central app and chat-widget files
should contain only the necessary orchestration.
These are areas for a patch series, not a requirement to combine each entire area
into one commit. Keep independent fixes separate so a future rebase can drop or
adapt the particular behavior that overlaps an upstream change.

Rebase regularly and inspect semantic changes even when a patch applies cleanly.
Remove a local patch when upstream provides equivalent behavior. Repository-local
resolution reuse is enabled with automatic staging disabled. Review reused
resolutions before staging, validate affected crates, and review UI snapshots
after each sync. Follow the recovery-ref and explicit push-lease procedure in
[fork-workflow.md](fork-workflow.md). `main` carries the fork patch stack.

See `PERFORMANCE_LOG.md` for measurement boundaries and migration validation.

## Validation record

Validation ran on arm64 macOS with the workspace's Rust 1.95.0 toolchain.

| Area                                                                                                           | Result                           |
| -------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| Provider, authentication, HTTP/API transport, model catalog, app-server protocol                               | 989 passed, 1 skipped            |
| App-server provider-scoped `model/list` integration                                                            | 8 passed                         |
| CLI binary tests                                                                                               | 288 passed                       |
| Core client/timing, telemetry, shell display parsing, thread storage, and app-server subscriber/listener tests | 504 passed                       |
| Full TUI library suite                                                                                         | 4,531 passed, 2 skipped          |
| Final provider-switch and resume compatibility coverage                                                        | 6 passed                         |
| Repository-wide argument-comment lint                                                                          | 947 targets checked successfully |

Stable and experimental app-server schemas, the configuration schema, and the
Bazel lock refresh completed. The dependency changes reuse existing workspace
dependencies, so the Bazel module lockfile did not change. Scoped `just fix`
completed with non-fatal unused-code and argument-count warnings; `just fmt`
completed successfully. Unrelated Clippy cleanup was excluded from the migration.

The rebuilt developer CLI passed `--version` and an interactive check of startup,
the OpenAI/Copilot provider picker, the current model catalog, returning to the
composer, and clean shutdown. Copilot authentication and transport were validated
with mocks; live GitHub device login and Linux/Windows execution were not exercised.

All intentional snapshot updates were reviewed. The full suite ran before the
final resume-hook adjustment; the six focused tests and argument-comment lint
then validated that adjustment. Tests precede the final Clippy/formatting pass,
as required by this repository's instructions.
