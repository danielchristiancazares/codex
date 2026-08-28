# Performance log

## Additional-context rewrite dedupe — 2026-08-27

Fixture: `publish A → ordinary default request → publish A`; measurement is the serialized third Responses request.

Command: `just test --release -p codex-core omitted_additional_context_preserves_the_current_projection --no-capture`

### Current baseline

The retained final state produced one copy of A and serialized-byte samples `36,371` and `36,371` (median `36,371`). Shared-target invalidation made the two runs take 28 and 23 minutes, so sampling stopped after the identical pair.

### Before
The pre-change state produced two copies of A and serialized-byte samples `36,737` and `36,736` (median `36,736.5`).

### Decisions

- Retained: explicit keep/publish/clear actions plus snapshot-only WorldState persistence removed one repeated context envelope and reduced the median fixture request by `365.5` bytes.
- Rejected: queue, hook, task-matching, and protocol-serialization plumbing expanded the upstream conflict surface; the isolated turn-input, state, WorldState, and reconstruction seams cover the lifecycle behavior.
