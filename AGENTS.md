# Codex personal fork instructions

This repository is a single-maintainer fork of OpenAI Codex. Keep fork-specific changes small and
easy to reconcile with future upstream updates.

## Fork workflow

- Your operator directing the current task is the decision authority for product behavior, validation,
  commits, pushes, and releases in this fork.
- `origin` is the maintained personal repository. `upstream` is OpenAI's source repository and is
  used for reference and synchronization.
- OpenAI-authored code, documentation, and inherited conventions provide technical context. Local
  work follows a direct maintainer sequence: inspect, implement, validate in proportion to risk,
  review the resulting diff, commit or push when requested, and report what was completed.

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

## Rust/codex-rs

In the codex-rs folder where the rust code lives:

- Crate names are prefixed with `codex-`. For example, the `core` folder's crate is named `codex-core`
- When using format! and you can inline variables into {}, always do that.
- Treat existing checks for `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` and
  `CODEX_SANDBOX_ENV_VAR` as test-environment guards; understand their sandbox behavior before
  changing them.
- Always collapse if statements per https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if
- Always inline format! args when possible per https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args
- Use method references over closures when possible per https://rust-lang.github.io/rust-clippy/master/index.html#redundant_closure_for_method_calls
- Avoid bool or ambiguous `Option` parameters that force callers to write hard-to-read code such as `foo(false)` or `bar(None)`. Prefer enums, named methods, newtypes, or other idiomatic Rust API shapes when they keep the callsite self-documenting.
- When you cannot make that API change and still need a small positional-literal callsite in Rust, follow the `argument_comment_lint` convention:
  - Use an exact `/*param_name*/` comment before opaque literal arguments such as `None`, booleans, and numeric literals when passing them by position.
  - A method's sole non-self argument is exempt when the method and parameter names match, such as `.enabled(false)` for `fn enabled(&self, enabled: bool)`.
  - Do not add these comments for string or char literals unless the comment adds real clarity; those literals are intentionally exempt from the lint.
  - The parameter name in the comment must exactly match the callee signature.
  - Run `just argument-comment-lint` when the change touches relevant call sites. Its first Bazel
    invocation can be slow; report the check explicitly when it is skipped or unavailable.
- When possible, make `match` statements exhaustive and avoid wildcard arms.
- Newly added traits should include doc comments that explain their role and how implementations are expected to use them.
- Discourage both `#[async_trait]` and `#[allow(async_fn_in_trait)]` in Rust traits.
  - Prefer native RPITIT trait methods with explicit `Send` bounds on the returned future.
  - Preferred trait shape:
    `fn foo(&self, ...) -> impl std::future::Future<Output = T> + Send;`
  - Implementations may still use `async fn foo(&self, ...) -> T` when they satisfy that contract.
  - Do not use `#[allow(async_fn_in_trait)]` as a shortcut around spelling the future contract explicitly.
- When writing tests, prefer comparing the equality of entire objects over fields one by one.
- Do not add tests for values that are statically defined.
- Do not add negative tests for logic that was removed.
- Use `docs/` for durable fork-owned architecture notes, audits, runbooks, decisions, and behavior
  that differs from OpenAI upstream. Keep app-server API documentation aligned with the app-server
  guidance below. Avoid duplicating upstream public documentation when a link and a concise record
  of the fork's divergence are sufficient.
- Prefer private modules and explicitly exported public crate API.
- If you change `ConfigToml` or nested config types, run `just write-config-schema` to update `codex-rs/core/config.schema.json`.
- When working with MCP tool calls, prefer using `codex-rs/codex-mcp/src/mcp_connection_manager.rs` to handle mutation of tools and tool calls. Aim to minimize the footprint of changes and leverage existing abstractions rather than plumbing code through multiple levels of function calls.
- Do not call `reset_client_session` unnecessarily; let the incremental check logic decide whether to reuse the previous request.
- If you change Rust dependencies (`Cargo.toml` or `Cargo.lock`), run `just bazel-lock-update` from the
  repo root to refresh `MODULE.bazel.lock`, and include that lockfile update in the same change.
  Bazel validation expects the Cargo and module lockfiles to stay aligned.
- Bazel does not automatically make source-tree files available to compile-time Rust file access. If
  you add `include_str!`, `include_bytes!`, `sqlx::migrate!`, or similar build-time file or
  directory reads, update the crate's `BUILD.bazel` (`compile_data`, `build_script_data`, or test
  data) or Bazel may fail even when Cargo passes.
- Do not create small helper methods that are referenced only once.
- For tracing async work, instrument the function or method definition with
  `#[tracing::instrument(...)]` instead of attaching spans to futures with
  `.instrument(...)` at call sites. Before adding instrumentation, check whether the callee—or
  the implementation method it immediately delegates to—is already instrumented.
- Before editing, inspect the target file size and nearby modules.
- Avoid large modules:
  - Prefer adding new modules instead of growing existing ones.
  - Target Rust modules under 500 LoC, excluding tests.
  - If a file exceeds roughly 800 LoC, add new functionality in a new module instead of extending
    the existing file unless there is a strong documented reason not to.
  - This rule applies especially to high-touch files that already attract unrelated changes, such
    as `codex-rs/tui/src/app.rs`, `codex-rs/tui/src/bottom_pane/chat_composer.rs`,
    `codex-rs/tui/src/bottom_pane/footer.rs`, `codex-rs/tui/src/chatwidget.rs`,
    `codex-rs/tui/src/bottom_pane/mod.rs`, and similarly central orchestration modules.
  - When extracting code from a large module, move the related tests and module/type docs toward
    the new implementation so the invariants stay close to the code that owns them.
  - Avoid adding new standalone methods to `codex-rs/tui/src/chatwidget.rs` unless the change is
    trivial; prefer new modules/files and keep `chatwidget.rs` focused on orchestration.
- Let Rust commands finish through ordinary lock contention; they may wait on the workspace lock.

After code changes:

1. Use the smallest crate or test filter that covers the change.
2. Before finalizing a large change, use `just fix` and `just fmt`
   Do not rerun tests after the final `just fix` and `just fmt`.

- `just test` takes a long time so try to strategize your changes to streamline `just test` runs.
- Before a long validation run, ensure the required repository tools are installed: `just`,
  `cargo-nextest`, `dotslash`, and `uv`; TUI snapshot work also needs `cargo-insta`.
- Give automated subtasks the same repository validation commands; prefer root `just` recipes in
  place of direct `cargo test`, `cargo fmt`, or `cargo clippy` commands.
- Targeted `argument-comment-lint` runs use a prebuilt package that does not support Intel macOS.
  On `x86_64-apple-darwin`, use the repo-wide Bazel path or record the targeted check as
  unavailable.
- When multiple automated tasks edit the same worktree, partition them by non-overlapping files
  and defer final tests, Clippy, and formatting until every edit is complete. A half-written file
  from another task can make an otherwise unrelated targeted test fail.

### Intel macOS rusty_v8 fallback

On this x86_64 macOS host, the `v8` 150.4.0 prebuilt archive may return HTTP 404. Do not retry the
same download. If the verified local source build exists, pass both its archive and generated
sandbox binding to Cargo-backed `just` commands:

```sh
env \
  V8_FROM_SOURCE=0 \
  V8_FORCE_DEBUG=0 \
  MACOSX_DEPLOYMENT_TARGET=12.0 \
  RUSTY_V8_ARCHIVE=/Users/daniel/rusty_v8/target/release/gn_out/obj/librusty_v8.a \
  RUSTY_V8_SRC_BINDING_PATH=/Users/daniel/rusty_v8/target/release/gn_out/src_binding.rs \
  just test -p <crate>
```

- Verify both files are nonempty and `lipo -info "$RUSTY_V8_ARCHIVE"` reports `x86_64`; the archive
  alone is insufficient for the sandbox-enabled build.
- Use the same environment for `just fix`.
- Before rebuilding rusty_v8 from source, check available disk space. If cleanup is necessary, use
  a targeted Cargo profile cleanup rather than deleting source or unrelated caches.

## Install and release packaging

- Distinguish a developer build from an installable release artifact. Before giving install
  instructions, inspect `scripts/install/`, `codex-cli/`, and the current release workflow; do not
  present `cargo install` of one crate as equivalent to the packaged distribution.
- Use `scripts/build_codex_package.py` and `scripts/codex_package/layout.py` as the source of truth
  for package construction. Do not hand-roll a parallel bundle layout.
- A canonical package includes `codex-package.json`, `bin/`, `codex-resources/`, and `codex-path/`.
  Release staging should pass already built or signed binaries to the package builder rather than
  silently rebuilding different artifacts.
- For cross-target builds, verify every bundled executable, including `rg`, zsh, and platform
  helpers, matches the requested target. Only execute smoke tests on a compatible host
  architecture; otherwise use target-aware static layout and binary checks.

## Performance changes

Performance work must be evidence-driven:

1. Record a warmed release-mode baseline before editing.
2. Change and measure one candidate at a time with identical fixtures and sampling.
3. Keep only repeatable improvements outside run-to-run variance; revert regressions and noise.
4. Add correctness coverage for retained behavior changes.
5. Update `PERFORMANCE_LOG.md` with a current-baseline section, retained wins, rejected experiments,
   exact commands, fixtures, and medians so future sessions do not repeat failed work.
6. Before declaring completion, rerun the retained final state and make the current-baseline
   section describe that state. The final report must name the updated `PERFORMANCE_LOG.md` section
   and summarize retained and rejected experiments.

For TUI performance, separate widget computation, buffer diff/ANSI serialization, and terminal or
VT100 end-to-end costs. Use the requested benchmark as the primary performance evidence.

## Compatibility and context invariants

Keep crate API surfaces small and avoid proliferating test-only helpers.

### Model-visible context

Codex maintains a context (history of messages) that is sent to the model in inference requests.

1. Build history incrementally and preserve existing items.
2. Avoid frequent changes to context that cause cache misses.
3. Give every injected item a bounded size and a hard cap.
4. Keep individual items at or below 10K tokens.
5. Give new items that can exceed 1K tokens a focused size and bounds review.
6. Define injected fragments as structs in `core/context` that implement
   `ContextualUserFragment`.

### Breaking changes

Search for breaking changes in external integration surfaces:

- app-server APIs
- raw response item events (`rawResponseItem/*`), even while experimental
- CLI parameters
- configuration loading
- resuming sessions from existing rollouts

### Compatibility tracing

Before changing an event, protocol type, request pipeline, or authentication path, search every
producer and consumer.

- For TUI `AppEvent` changes, inspect `tui/src/session_log.rs`, replay fixtures, exhaustive matches,
  and snapshots. Session logging currently derives generic event names from `Debug` output, so
  changing a variant between unit, tuple, and struct forms can change recorded compatibility.
- For model transport/auth changes, trace request construction through normalization,
  serialization/compression, authentication, and every retry path. A mutation hook that runs after
  the body is encoded does not affect the transmitted request; retries should reuse the intended
  prepared representation.
- Keep authentication providers and token managers shared for the lifetime expected by their
  callers. Do not hold locks across network or device-flow awaits; refresh must be single-flight
  and cancellation-safe, and invalidation must target the exact rejected token generation or
  fingerprint so a delayed failure cannot evict newer state.
- Treat credential persistence as best-effort after valid credentials are stored in memory. Failed
  writes or deletions must not cause repeated login or reload a rejected credential. Resolve
  credential paths through `codex_utils_home_dir::find_codex_home()` or an injected resolved
  `codex_home`, never directly from `HOME` or `USERPROFILE`.

## TUI conventions

See `codex-rs/tui/styles.md`.

- Prefer Ratatui's `Stylize` helpers, such as `"text".dim()`, `.bold()`, `.cyan()`, and
  `.underlined()`.
- Use `"text".into()` for spans and `vec![...].into()` for lines when the target type is clear.
  Use `Line::from(...)` or `Span::from(...)` when inference is ambiguous.
- Runtime-computed styles may use `Span::styled` or `Span::from(text).set_style(style)`.
- Use the default foreground instead of hardcoded white.
- Follow file-local conventions and avoid style-only churn between equivalent forms. Prefer the
  form that stays compact after rustfmt.

### Text wrapping

- Always use textwrap::wrap to wrap plain strings.
- If you have a ratatui Line and you want to wrap it, use the helpers in tui/src/wrapping.rs, e.g. word_wrap_lines / word_wrap_line.
- If you need to indent wrapped lines, use the initial_indent / subsequent_indent options from RtOptions if you can, rather than writing custom logic.
- If you have a list of lines and you need to prefix them all with some prefix (optionally different on the first vs subsequent lines), use the `prefix_lines` helper from line_utils.

## Tests

Changes to agent behavior require integration coverage under `codex-rs/core/tests/suite` using
`test_codex`. Reuse existing test-support helpers and keep test-only APIs out of production code.

### Test module organization

- Put new test modules in descriptive sibling files and connect them with an explicit `#[path]`:

  ```rust
  #[cfg(test)]
  #[path = "parser_tests.rs"]
  mod tests;
  ```

- Leave existing inline test modules in place unless the active change gives a separate reason to
  move them.

### Snapshot tests

This repo uses snapshot tests (via `insta`), especially in `codex-rs/tui`, to validate rendered output.

**Requirement:** any change that affects user-visible UI (including adding new UI) must include
corresponding `insta` snapshot coverage (add a new snapshot test if one doesn't exist yet, or
update the existing snapshot). Review and accept snapshot updates in the same change so UI impact
stays explicit and future diffs remain visual.

When UI or text output changes intentionally, update the snapshots as follows:

- Run tests to generate any updated snapshots:
  - `just test -p codex-tui`
- Check what’s pending:
  - `cargo insta pending-snapshots -p codex-tui`
- Review changes by reading the generated `*.snap.new` files directly in the repo, or preview a specific file:
  - `cargo insta show -p codex-tui path/to/file.snap.new`
- Only if you intend to accept all new snapshots in this crate, run:
  - `cargo insta accept -p codex-tui`

Install the snapshot tool with `cargo install --locked cargo-insta` when needed.

### Test assertions

- Use `pretty_assertions::assert_eq` for clearer diffs when the module does not already provide an
  equivalent.
- Avoid mutating process environment in tests; prefer passing environment-derived flags or dependencies from above.

### Spawning workspace binaries in tests (Cargo vs Bazel)

- Prefer `codex_utils_cargo_bin::cargo_bin("...")` over `assert_cmd::Command::cargo_bin(...)` or `escargot` when tests need to spawn first-party binaries.
  - Under Bazel, binaries and resources may live under runfiles; use `codex_utils_cargo_bin::cargo_bin` to resolve absolute paths that remain stable after `chdir`.
- When locating fixture files or test resources under Bazel, avoid `env!("CARGO_MANIFEST_DIR")`. Prefer `codex_utils_cargo_bin::find_resource!` so paths resolve correctly under both Cargo and Bazel runfiles.

### Integration tests

#### codex_core integration testing

- Prefer the utilities in `core_test_support::responses` when writing end-to-end Codex tests.
- Use `TestCodexBuilder::build_with_auto_env()` by default to ensure that new tests work with
  foreign app/exec operating systems.
- All `mount_sse*` helpers return a `ResponseMock`; hold onto it so you can assert against outbound `/responses` POST bodies.
- Use `ResponseMock::single_request()` when a test should only issue one POST, or `ResponseMock::requests()` to inspect every captured `ResponsesRequest`.
- `ResponsesRequest` exposes helpers (`body_json`, `input`, `function_call_output`, `custom_tool_call_output`, `call_output`, `header`, `path`, `query_param`) so assertions can target structured payloads instead of manual JSON digging.
- Build SSE payloads with the provided `ev_*` constructors and the `sse(...)`.
- Prefer `wait_for_event` over `wait_for_event_with_timeout`.
- Prefer `mount_sse_once` over `mount_sse_once_match` or `mount_sse_sequence`.

#### app-server integration testing

- Tests should exercise app-server's public JSON-RPC API.
- Use similar server mocking as for core integration tests.
- Use `TestAppServer::builder().build()` and `TestAppServer::send_thread_start_request_with_auto_env()`
  by default so new tests work with foreign app/exec operating systems.

## App-server API Development Best Practices

These guidelines apply to app-server protocol work in `codex-rs`, especially:

- `app-server-protocol/src/protocol/common.rs`
- `app-server-protocol/src/protocol/v2.rs`
- `app-server/README.md`

### Core Rules

- All active API development should happen in app-server v2. Do not add new API surface area to v1.
- Follow payload naming consistently:
  `*Params` for request payloads, `*Response` for responses, and `*Notification` for notifications.
- Expose RPC methods as `<resource>/<method>` and keep `<resource>` singular (for example, `thread/read`, `app/list`).
- Always expose fields as camelCase on the wire with `#[serde(rename_all = "camelCase")]` unless a tagged union or explicit compatibility requirement needs a targeted rename.
- Always expose string enum values as camelCase on the wire with matching serde and TS `rename_all = "camelCase"` annotations unless an explicit compatibility requirement needs targeted renames.
- Exception: config RPC payloads are expected to use snake_case to mirror config.toml keys (see the config read/write/list APIs in `app-server-protocol/src/protocol/v2.rs`).
- Always set `#[ts(export_to = "v2/")]` on v2 request/response/notification types so generated TypeScript lands in the correct namespace.
- Never use `#[serde(skip_serializing_if = "Option::is_none")]` for v2 API payload fields.
  Exception: client->server requests that intentionally have no params may use:
  `params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>`.
- Keep Rust and TS wire renames aligned. If a field or variant uses `#[serde(rename = "...")]`, add matching `#[ts(rename = "...")]`.
- For discriminated unions, use explicit tagging in both serializers:
  `#[serde(tag = "type", ...)]` and `#[ts(tag = "type", ...)]`.
- Prefer plain `String` IDs at the API boundary (do UUID parsing/conversion internally if needed).
- Timestamps should be integer Unix seconds (`i64`) and named `*_at` (for example, `created_at`, `updated_at`, `resets_at`).
- For experimental API surface area:
  use `#[experimental("method/or/field")]`, derive `ExperimentalApi` when field-level gating is needed, and use `inspect_params: true` in `common.rs` when only some fields of a method are experimental.

### Client->server request payloads (`*Params`)

- Every optional field must be annotated with `#[ts(optional = nullable)]`. Do not use `#[ts(optional = nullable)]` outside client->server request payloads (`*Params`).
- Optional collection fields (for example `Vec`, `HashMap`) must use `Option<...>` + `#[ts(optional = nullable)]`. Do not use `#[serde(default)]` to model optional collections, and do not use `skip_serializing_if` on v2 payload fields.
- When you want omission to mean `false` for boolean fields, use `#[serde(default, skip_serializing_if = "std::ops::Not::not")] pub field: bool` over `Option<bool>`.
- For new list methods, implement cursor pagination by default:
  request fields `pub cursor: Option<String>` and `pub limit: Option<u32>`,
  response fields `pub data: Vec<...>` and `pub next_cursor: Option<String>`.

### Development Workflow

- Update app-server docs/examples when API behavior changes (at minimum `app-server/README.md`).
- Regenerate schema fixtures when API shapes change:
  `just write-app-server-schema`
  (and `just write-app-server-schema --experimental` when experimental API fixtures are affected).
- Validate with `just test -p codex-app-server-protocol`.
- Avoid boilerplate tests that only assert experimental field markers for individual
  request fields in `common.rs`; rely on schema generation/tests and behavioral coverage instead.

## Syncing with OpenAI upstream

For requests to update this fork from OpenAI:

1. Inspect `git status --short`, remotes, the current branch and tracking branch, linked worktrees,
   stashes, merge-base, and ahead/behind counts.
2. Preserve every staged, unstaged, and untracked change before moving a branch. Record the current
   `origin/main` tip and create a dated backup ref for broad or history-changing integrations.
3. Fetch from `upstream` and integrate through the method requested by the user. Treat
   `upstream/main` as source input and `origin/main` as the fork's maintained product history.
4. Port behavior semantically into the fork's exhaustive enums and boundary adapters, even when an
   upstream patch applies textually. Preserve the fork's stronger state representation. Translate
   nested or ambiguous `Option` values, boolean-like state encodings, and raw strings for closed
   state sets into named states at their integration boundary.
5. Review every conflict and newly introduced upstream abstraction before choosing where local
   behavior belongs. Re-check that an older integration seam still controls the current request or
   rendering path, then validate affected crates first.

Use `upstream` for fetch and reference work. Push ordinary fork work to `origin`; any contribution
or push toward `openai/codex` requires an explicit user request.

## Commit and push completion

When the user requests a commit or push, treat it as part of the task's completion criteria.

1. Re-read `git status --short` and classify every staged, unstaged, and untracked path as
   task-owned or unrelated before staging.
2. Stage task-owned paths with explicit pathspecs. Do not use `git add -A` or absorb unrelated
   worktree changes into the commit.
3. Review `git diff --cached --check`, the staged stat, and the staged patch before committing.
   A direct instruction to commit immediately may skip broader validation while path
   classification and staged-content review remain required.
4. Push without force unless the user explicitly requested history rewriting, then verify the
   branch's remote tracking ref contains the new commit. The requested commit or push must be
   complete before reporting the task complete.

## Python

This project uses Python 3+. You should not use the `__future__` module.

Check the nearest `pyproject.toml`'s `requires-python` before using point-release-specific features.

## Platform Support

Preserve Linux, macOS, and Windows portability in touched code unless the behavior is OS-specific.
Run validation available on the current host and report unverified platforms.
