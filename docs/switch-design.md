# Live account switching: fork integration notes

## Maintained behavior

The fork's command is `/switch`; `/sw` uniquely completes to it.
`/provider` is removed outright. No parsing aliases are maintained.

The existing provider picker now lists saved ChatGPT and GitHub Copilot
connections. A connection has a nickname, provider, and validated identity.
`/switch <nickname>` resolves an exact case-insensitive nickname.
`/switch add` starts the existing provider's browser or device-code login
in an isolated credential scope. Adding a login leaves the active account
selected until the operator chooses the new entry.

Switching keeps the app-server runtime alive. The existing provider-switch
flow creates a replacement thread with saved conversation history and restores
the unsent draft. It runs between turns in an idle local primary session.
Background terminals, active agents, unsaved conversation history, remote
workspaces, host-owned authentication, and ephemeral credential storage retain
their corresponding availability restrictions.

## Storage ownership

`codex-login` owns the registry in focused `connections` modules.

- Existing root OpenAI and Copilot stores are referenced in place.
- New logins use `connections/saved-<random-id>/`, with the provider's existing
  credential format and file/keyring/auto storage policy.
- Each completed login publishes bounded, non-secret `connection.json` metadata.
  Nicknames and connection IDs are validated at the storage boundary.
- OpenAI identity includes both user and workspace. Copilot identity is bound to
  the saved credential's machine ID.
- `connections/selected.json` records the last committed default.
- Failed or canceled logins remain unregistered; their isolated scope cannot
  replace the active credential store.
- Registration is serialized across processes. Refreshes use a credential-scope
  lease covering reread, exchange, and persistence, so another process can reuse
  an already rotated token. File writes use atomic replacement.
- Logout clears the selected scope's registration and its matching startup
  default. Other saved accounts remain intact.
- Ordinary persistent login selects the configured store again. Its successful
  write clears the saved startup choice while retaining the saved accounts;
  app-server login completion rebinds the live manager to that configured store.
  CLI logout resolves the saved default before revoking and removing credentials.

The real Codex home continues to own configuration, sessions, and workspace
resources. Only credential storage and account-specific model caches use the
selected scope. The runtime binds its own selection; another app-server's
selection affects the startup default without retargeting this runtime.

## Live handoff

The additive v2 `savedConnection/manage` endpoint lives in a dedicated protocol
module. Its server implementation is separate from ordinary account/login
handlers.

1. Prepare the destination using a separate auth manager and its model catalog.
   The active manager remains bound to the source.
2. Select the destination with a client-minted, connection-owned receipt.
   The server rechecks idle prerequisites and activates credentials.
3. The TUI uses the existing thread fork/start APIs, validates the replacement,
   and detaches the source. A failure restores the source credentials.
4. Commit once the replacement is usable. Persist the selected default, then
   attach the replacement to the TUI and restore the draft. A default-write
   failure leaves the live selection in place.
5. An abandoned client handoff requests restoration. The server also restores
   an unfinished receipt when its owning connection closes. Repeated settlement
   requests are checked against the retained receipt.

A single private server-side handoff owner controls activation, receipt checks,
settlement, and disconnect recovery. The TUI owns presentation and invokes the
existing history-preserving transition. Account-change consequences reuse
existing cloud configuration, plugin-cache, and account-notification handling.

## Upstream-facing seams

| Area                         | Integration                                                                   |
| ---------------------------- | ----------------------------------------------------------------------------- |
| TUI command catalog/dispatch | Rename the command and route to fork-owned picker/transition modules          |
| TUI application events       | One connection-action route and login-completion hook                         |
| Login manager                | Selected credential scope, startup restoration, refresh/logout lease hooks    |
| File auth storage            | Atomic writer adapter; credential payload stays unchanged                     |
| Model provider               | `ScopedModelCatalog` owns provider/credential binding and reuse               |
| Thread manager               | Construct the catalog owner and delegate catalog selection at thread creation |
| App-server catalog           | Request the catalog bound to the active provider and credential scope         |
| App-server account processor | Register the handoff owner and delegate the new endpoint                      |
| App-server dispatch/cleanup  | One endpoint dispatch and one disconnect-recovery call                        |

Keep scope comparison and cache replacement inside `ScopedModelCatalog`.
Keep receipt state and activation/recovery inside the dedicated handoff owner.
The upstream thread manager carries neither parallel cache-binding fields nor
the binding-selection algorithm.

`AuthDotJson`, `ConfigToml`, existing login RPC payloads, and rollout formats
retain their schemas. Per-account model/reasoning preferences are outside this
implementation; the existing compatible-model selection rules apply.

## Reviewable stages

This feature exceeds the repository's 800-line guidance. Coherent review units
from the actual dependency graph are:

1. Saved credential scopes, registration, atomic persistence, and scoped refresh
   ownership in `codex-login`, including credential-isolation tests.
2. Provider-owned catalog binding and the small thread/catalog integration hooks.
3. Connection RPC types and server-owned handoff/login coordination, with generated
   schemas and public JSON-RPC tests.
4. The `/switch` picker, nickname/login UI, and existing transition integration,
   with reviewed snapshots and provider-switch regressions.

The credential-scope stage is the smallest foundation that can land first.
The unified command becomes usable when all four stages are present. Keep the
operator's concurrent keyring and fork-workflow changes outside these stages.

Validation covers four OpenAI accounts, same-provider changes, Copilot saved
login, refresh-token rotation, process restart, source restoration, canceled
login, target-auth model discovery/inference, conversation history, and the UI.
Use scoped root `just test` recipes, regenerate API schemas, and finish with
scoped `just fix` followed by `just fmt`.

### Recorded validation on Windows

- All 228 login tests and 52 switching-related TUI tests passed.
- All four public saved-connection API integration tests passed, including a
  history-preserving handoff with target inference credentials and recovery in
  the same app-server process.
- Stable and experimental schema fixtures passed after regeneration.
- All 180 scoped core/model-provider checks passed.
- A broader TUI run exposed ten failures in other rendering, status, and
  platform-specific fixtures. Their existing snapshots remain unchanged.
- The operator's full-workspace run reached `codex-voice-host`, which requires
  the Windows GStreamer development libraries and `pkg-config`. Excluding that
  crate keeps the native audio dependency outside the workspace test build.

The login fixture's Windows `more +1` command requires the system implementation;
the local uutils shim shadowed it. Login validation used a child-process-only
System32-first PATH, leaving the operator's environment unchanged.
