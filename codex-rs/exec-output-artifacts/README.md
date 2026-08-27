# Exec-output artifacts

This crate owns bounded, session-authorized copies of unified-exec stdout and
stderr that stay outside ordinary model conversation history. Its namespace,
reference scheme, storage root, and model tool are deliberately specific to
execution output. Other artifact products keep their own lifecycle and API.

The generalized boundary is small:

1. raw exec owns process lifecycle and artifact-backed streams;
2. `exec_output_query` owns bounded metadata, head, tail, range, line, and
   search views; and
3. diagnostics, workflow semantics, retries, workspace observation, and
   caching remain above exec.

## Existing execution path

Before this crate was introduced, model-requested execution followed this path:

1. `core/src/tools/handlers/unified_exec/exec_command.rs` selected the target
   environment, resolved the working directory, ran hooks, evaluated approval,
   and submitted an `ExecCommandRequest`.
2. `core/src/tools/runtimes/unified_exec.rs` preserved the approval and sandbox
   boundary, constructed the final `ExecRequest`, and spawned either a local
   process or an exec-server process.
3. `core/src/unified_exec/process.rs` drained local stdout and stderr, merged
   them into one broadcast stream, and retained a bounded aggregate
   `HeadTailBuffer`. Exec-server output carried a stream label but was merged at
   this boundary as well.
4. `core/src/unified_exec/process_manager.rs` waited for an initial output
   window and returned `ExecCommandToolOutput`. Running processes remained in a
   session-local process map and were polled through `write_stdin`.
5. `core/src/tools/context.rs` decoded and token-truncated the collected bytes
   into the model-visible function-call output. That response item was appended
   to conversation history and persisted in the rollout.
6. `core/src/unified_exec/async_watcher.rs` streamed bounded output deltas and,
   after process exit, emitted `ItemCompleted(CommandExecution)`. Session event
   delivery persisted that completion event to the rollout before delivering
   it to clients.
7. App-server v2 translated the command item for rich clients. Its standalone
   `command/exec` API already accepted an argv vector and independently
   preserved stdout/stderr stream labels.

The process manager and output buffers are session memory. A local process does
not survive application restart. Completed rollout events do survive. Existing
polling and serialized rollout shapes are compatibility surfaces.

## Security boundaries

Execution authorization remains outside this crate. Environment selection,
working-directory resolution, hooks, approval policy, additional permissions,
network policy, and sandbox transformation all complete before process launch.
Artifact capture cannot grant execution or filesystem authority.

Raw process bytes are ephemeral input. The unified-exec integration converts
captured bytes to a model-eligible representation and applies the existing
best-effort secret sanitizer before committing content. Only sanitized content
is durable. Binary-looking streams are represented by a deterministic,
non-content summary; their raw bytes are never committed or returned through
artifact queries.

Every artifact belongs to:

- one thread identifier;
- one execution environment identifier; and
- one fingerprint of the environment's workspace roots.

References are random opaque capabilities and contain no filesystem path.
Storage lookup is rooted in the current thread directory, and query access also
requires an exact workspace-authority fingerprint. Unknown, foreign, expired,
incomplete, or corrupt artifacts fail closed.

Content and metadata are committed through temporary files and replacement
renames.
Incomplete manifests make interrupted writes observable after restart without
making partial content readable. Retention and per-artifact/per-thread quotas
bound disk and memory use. Quota-sensitive mutations and retention cleanup hold
a store-wide cross-process lock and rebuild usage from disk. Accounting covers
file contents plus a fixed allowance for every file and directory entry, and
thread directories are created lazily and removed once empty.

## Context and compatibility boundary

The retained sanitized content lives in the artifact store. Ordinary model
history receives only:

- a bounded preview;
- an opaque reference;
- content and presentation byte counts;
- digest, media, encoding, lifecycle, and truncation metadata.

Artifact queries return bounded slices with source offsets. Identical slices
carry a stable digest and may be represented by a duplicate receipt within one
turn and history-rewrite epoch. Callers can request the data again after
compaction or when rebuilding context.

Descriptors appear only in model-facing `exec_command` and `write_stdin`
results. Shared command events, rollout items, app-server protocol types, and
raw output deltas retain their existing shapes and behavior. The feature uses
the under-development `exec_output_artifacts` key; upstream's native
`artifact` feature remains independent.

## Extension contract

Unified exec reserves references, collects bounded raw input, transforms it
into a sanitized model-eligible payload, and finalizes the reservation.
Capture and persistence are advisory: a storage, quota, integrity, or timeout
failure cannot change the process exit status or turn a successful command into
a tool failure. Unproven or lossy capture is published as truncated.

Queries support metadata, head, tail, byte ranges, line ranges, and bounded
literal or regular-expression search. A truncated prefix cannot answer a tail
query. Command-specific diagnostics, test semantics, workspace observation,
retry policy, and result caching belong to higher-level workflows or typed
adapters.
