# Codex repository instructions

Read the nearest `AGENTS.md` before editing. The root file contains the authoritative Rust,
testing, TUI, protocol, and app-server rules; nested files add local requirements.

## Repository shape

- `codex-rs/` is the primary implementation and Cargo workspace.
- `codex-rs/cli/` contains the Rust CLI entry point.
- `codex-rs/core/` contains shared agent business logic. Resist adding new concepts here; prefer
  an existing focused crate or a new crate with a small integration seam.
- `codex-rs/tui/` is the Ratatui interface, event loop, rendering, transcript, and widget code.
  `codex-rs/tui/src/bottom_pane/AGENTS.md` requires state-machine documentation to stay aligned
  with chat-composer and paste-burst behavior.
- `codex-rs/protocol/` defines shared internal and external types. Keep dependencies minimal and
  material business logic elsewhere.
- `codex-rs/app-server/` and `codex-rs/app-server-protocol/` expose the JSON-RPC API used by rich
  clients. New API development belongs in v2.
- `codex-rs/model-provider/`, `codex-rs/models-manager/`, and `codex-rs/codex-api/` cover provider
  selection, model catalogs, and model transport.
- `codex-cli/` is the thin npm launcher package, not the main implementation.
- The root `package.json` is for repository-wide maintenance tooling.

## Commands and working directory

Run root `just` recipes from the repository root. The root `justfile` automatically executes its
Rust recipes in `codex-rs/`.

```sh
# Build or run
cd codex-rs && cargo check -p codex-tui
just codex -- <args>

# Test one crate or one test/filter
just test -p codex-tui
just test -p codex-tui custom_terminal::tests::diff_buffers

# Lint and format
just fix -p codex-tui
just fmt
just fmt-check

# Benchmarks
just bench-smoke
just bench -- <divan-args>
```

- Never run `cargo test` directly; use `just test`.
- Use the smallest crate/test filter that covers the change.
- For TUI-only work, do not run the workspace-wide test suite. Run
  `just test -p codex-tui`, then `just fix -p codex-tui`, then `just fmt`.
- Run the complete `just test` suite only when changes affect shared `common`, `core`, or
  `protocol` behavior, and ask the user before starting it.
- Do not rerun tests after the final `just fix` and `just fmt`.
- Before a long validation run, ensure the required repository tools are installed:
  `just`, `cargo-nextest`, `dotslash`, and `uv`; TUI snapshot work also needs `cargo-insta`.
- When parallel agents edit the same worktree, partition ownership by non-overlapping files and
  defer final tests, Clippy, and formatting until all edits have landed. A half-written file from
  another agent can make an otherwise unrelated targeted test fail.

## Performance changes

Performance work must be evidence-driven:

1. Record a warmed release-mode baseline before editing.
2. Change and measure one candidate at a time with identical fixtures and sampling.
3. Keep only repeatable improvements outside run-to-run variance; revert regressions and noise.
4. Add correctness coverage for retained behavior changes.
5. Update `PERF_LOG.md` with a current-baseline section, retained wins, rejected experiments, exact
   commands, fixtures, and medians so future sessions do not repeat failed work.

For TUI performance, separate widget computation, buffer diff/ANSI serialization, and terminal or
VT100 end-to-end costs. Do not substitute a large workspace test run for the requested benchmark.

## Placement and module size

Before editing, inspect the target file size and nearby modules.

- Target Rust modules below 500 lines where practical.
- If a target is already roughly 800 lines or larger, put new non-trivial functionality in a
  sibling module instead of extending the orchestration file.
- Keep the new module's tests and module/type documentation beside the implementation. New test
  modules belong in descriptive sibling `*_tests.rs` files using `#[path = "..."]`.
- Avoid adding standalone logic to high-churn files such as `tui/src/app.rs`,
  `tui/src/chatwidget.rs`, `tui/src/bottom_pane/chat_composer.rs`, and
  `tui/src/bottom_pane/footer.rs`; keep them focused on orchestration.
- Do not introduce a small helper that has only one call site; inline it unless extraction creates
  a real reusable abstraction.
- Keep crate public APIs narrow and avoid test-only production helpers.

## Compatibility tracing

Before changing an event, protocol type, request pipeline, or authentication path, search every
producer and consumer rather than updating only the obvious call site.

- For TUI `AppEvent` changes, inspect `tui/src/session_log.rs`, replay fixtures, exhaustive matches,
  and snapshots. Session logging currently derives generic event names from `Debug` output, so
  changing a variant between unit, tuple, and struct forms can change recorded compatibility.
- For model transport/auth changes, trace request construction through normalization,
  serialization/compression, authentication, and every retry path. A mutation hook that runs after
  the body is encoded does not affect the transmitted request; retries should reuse the intended
  prepared representation.
- Treat app-server APIs, raw response-item events, CLI parameters, config loading, and resumed
  rollout/session behavior as breaking-change surfaces.
- App-server additions go to v2, use the established camelCase wire conventions, update
  `app-server/README.md`, run `just write-app-server-schema` when shapes change, and validate with
  `just test -p codex-app-server-protocol`.
- User-visible TUI output changes require reviewed and accepted `insta` snapshots.

## Safe upstream synchronization

For requests to update this fork and reapply local behavior:

1. Inspect `git status --short`, remotes, branch/upstream configuration, merge-base, and divergence.
2. Preserve the complete staged, unstaged, and untracked behavior patch before moving the branch.
   Never use a destructive reset to discard local work.
3. Sync the upstream branch cleanly, then reapply the behavior semantically against current APIs.
   Do not assume the old integration seam still affects the current request or rendering path.
4. Review every conflict and newly introduced upstream abstraction before choosing where the local
   behavior belongs.
5. Validate the affected crates first; broaden only when shared crates require it.

Keep upstream-facing changes small and isolated so future release updates do not repeatedly fight
large patches in central modules.

## Repository-specific generated artifacts and guardrails

- After changing `Cargo.toml` or `Cargo.lock`, run `just bazel-lock-update` and include
  `MODULE.bazel.lock`.
- If compile-time Rust code reads a source-tree file (`include_str!`, `include_bytes!`,
  `sqlx::migrate!`, etc.), add the file to the crate's Bazel `BUILD.bazel` data.
- After changing `ConfigToml` or nested config types, run `just write-config-schema`.
- Never modify logic related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` or
  `CODEX_SANDBOX_ENV_VAR`.
- Keep behavior portable across Linux, macOS, and Windows unless it is explicitly platform-specific.
