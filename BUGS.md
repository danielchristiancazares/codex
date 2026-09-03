# Avoidable LLM Token Consumption Bugs

Active list of demonstrated model-token defects. Each entry has a reachable current-HEAD
mechanism that directly duplicates model-visible content, requests output no consumer
uses, starts an avoidable inference, or predictably forces a redundant follow-up,
replay, or compaction.

Defensive work without established direct burn is tracked in
[HARDENING.md](HARDENING.md). Product, quality, ordering, security, cache, and
provider-contract choices are tracked in
[DESIGN_DECISIONS.md](DESIGN_DECISIONS.md).

Detailed canonical merge analysis, source mappings, and preserved fleet adjudication
evidence are in [TOKEN_AUDIT_EVIDENCE.md](TOKEN_AUDIT_EVIDENCE.md).

## Active token-burning bug facets

**Summary:** 41 active demonstrated bug facets. Facet labels are authoritative where the same
common-fix ID also appears in one of the other active documents. Completed findings retained in
the detailed record for traceability are excluded from this count.

**Review reconciliation:** Source review moved CF-119 to the provider-contract backlog
and promoted the directly demonstrated facets CF-019, CF-038, CF-045, CF-049, CF-077,
CF-086, CF-089, CF-093, CF-096, CF-100, CF-105, and CF-120. CF-075 remains a design
decision because eliminating its established second request changes the intentional
user-versus-agent input ordering contract. That review produced
`56 - 1 + 12 = 67` active facets. CF-092 has since been resolved by invalidating the
frozen MCP binding on tool-list notifications and session recovery. CF-001, CF-002, CF-003,
CF-005, CF-006, and CF-012 have also been confirmed fixed, leaving 60 active.
CF-007 and CF-008 have since been confirmed fixed by the per-turn Code Mode dispatch ownership
and centralized request-level reasoning-summary policy, leaving 58 active.
CF-009 has since been confirmed fixed by filtering inherited token-count events and recomputing
fork usage from the retained child history, leaving 57 active.
CF-010 has since been confirmed fixed by promoting surviving full WorldState and TurnContext
baselines during reconstruction without a user boundary, leaving 56 active.
CF-105 has since been confirmed fixed by rejecting negative catalog truncation limits at the
serde boundary and using a checked conversion to `TruncationPolicy`, leaving 55 active.
A HEAD verification sweep confirmed the remediations for CF-014, CF-015, CF-016, CF-017,
CF-018, CF-019, CF-025, CF-030, CF-044, CF-046, CF-077, CF-086, and CF-123 were already landed
by the batch remediation commit; each row's required verification test now passes at HEAD, so
those rows are marked Complete, leaving 42 active.
CF-082 has since been confirmed fixed by recording review findings once in the user-action
envelope while emitting the UI-facing AgentMessage turn item without a history record, leaving
41 active.

**Evidence convention:** “Generated,” “resent,” or “extra request” describes client-observed model work. Provider billing is claimed only where usage or a provider contract establishes it; otherwise the record explicitly marks billing as unknown or conditional.

**Impact tiers:** **High** means a full request, compaction, large payload, or multiplicative recurrence; **Medium** is bounded or feature/failure-gated but still material; **Low** is small per occurrence or a narrow interleaving. Tiers estimate token reduction, not correctness severity.

| Canonical ID | Title & Summary | Reachability / Trigger | Expected Token Impact | Primary Fix Seam |
| --- | --- | --- | --- | --- |
| CF-020 (stale-notice/re-expansion) | Tool output lifecycle re-expands on resume and retains stale notices | Resume under changed output policy or image retention | High | `codex-rs/core/src/session/mod.rs::prepare_conversation_items_for_history` |
| CF-021 (audio-accounting) | Remote V2 undercharges audio while normal history overcharges durationless audio | Remote V2 audio or valid durationless audio | High | `codex-rs/utils/audio/src/lib.rs::estimate_audio_token_count` |
| CF-022 | Temporary structured turn timeout fails to cancel active model inference | Temporary title/recap timeout or stale result | Medium | `codex-rs/tui/src/temporary_structured_request.rs::run_temporary_structured_turn` |
| CF-027 | Cold resume resets shared rollout budget ledger to zero | Rollout budget enabled + cold resume | High | `codex-rs/core/src/agent/control.rs::AgentControl` & `core/src/rollout_budget.rs` |
| CF-028 | Same-window resume re-arms one-shot reminder delivery state | Same-window resume after reminder delivery | Low | `codex-rs/core/src/state/session.rs::SessionState` & `apply_rollout_reconstruction` |
| CF-033 | Goal tool responses echo full objective and derived state | Goal create/update with nontrivial objective | Low | `codex-rs/ext/goal/src/tool.rs::GoalToolExecutor` |
| CF-034 (swallowed-accounting-error) | Swallowed goal accounting persistence error launches duplicate turn | Goal accounting persistence failure | High | `codex-rs/ext/goal/src/runtime.rs::account_active_goal_progress` |
| CF-038 (expired-lease duplicate-sampling) | Phase 1 Memory Leases Can Expire Before Queued Jobs Start | Memories; batch >=17; jobs exceed one-hour lease | Medium | `memories/write/src/phase1.rs` |
| CF-045 (checkpoint-retired discovery-schema) | Memory Phase 1 Reuploads Discovery Schemas Retired by Compaction | Memories + compacted rollout with search schemas | High | `ToolSearchCall` |
| CF-049 (ordinary prompt-history citation) | Parsed Memory Citations Remain in Ordinary Model History | Assistant response contains parsed memory citation | Low | Preserve the raw response for rollout provenance, but strip hidden citation syntax in the prompt, compaction, and memory-extraction projections using the already parsed structured citation metadata |
| CF-052 | Rollback resurrects asynchronous hook context from deleted turns | Async hook completes after rollback | Medium | `codex-rs/core/src/session/handlers.rs::thread_rollback` |
| CF-054 | Environment World State emits complete bundle on any field change | One common environment field changes | Low | `codex-rs/core/src/context/world_state/environment.rs::EnvironmentsState::render_diff` |
| CF-056 | Guidance sections re-emit stale instructions across silent re-enable | Guidance disabled then re-enabled before compaction | Medium | `codex-rs/core/src/context/world_state/mod.rs::WorldState::render_history_diff` |
| CF-060 | Skill catalog updates append full current body instead of deltas | Skill catalog changes by one/few entries | Medium | `codex-rs/ext/skills/src/world_state.rs` & `world_state_catalogs.rs` |
| CF-063 (exact common guidance) | Multi-authority skill catalogs repeat exact common guidance | Multiple skill authorities share common guidance | Low | `codex-rs/ext/skills/src/render.rs::render_combined_available_skills` |
| CF-067 | Unified Exec retains orphaned process handles across interrupt and resume | Unified Exec handle after interrupt/resume | Medium | `codex-rs/core/src/unified_exec/process_manager.rs` |
| CF-069 | Non-TTY Unified Exec sessions advertise unsupported stdin interaction | Non-TTY exec followed by `write_stdin` | Medium | `codex-rs/core/src/tools/handlers/shell_spec.rs` & `process_manager.rs` |
| CF-071 | Code Mode resume restores dead cell IDs forcing failed wait continuations | Yielded Code Mode cell across cold resume | Medium | `codex-rs/core/src/tools/code_mode/mod.rs` & `code-mode-runtime` |
| CF-073 | Fatal asynchronous tool errors are swallowed and trigger empty follow-ups | Fatal parallel/asynchronous tool future | High | `codex-rs/core/src/session/turn.rs::drain_in_flight` |
| CF-076 | Reused V1 subagents fail to rearm completion watchers | Second or later turn on reused V1 agent | High | `codex-rs/core/src/agent/control.rs` |
| CF-078 | Nonterminal subagent errors are published as terminal failures | Nonterminal agent `ErrorEvent` | Medium | `codex-rs/core/src/agent/status.rs::agent_status_from_event` |

| CF-083 | Reusable Guardian delta turns re-emit root evidence and advance on parse failure | Reusable Guardian follow-up or parse retry | High | `codex-rs/core/src/guardian/prompt.rs` & `review_session.rs` |
| CF-088 | Guardian V2 classifier prompt duplicates current tool action in transcript | Guardian V2 reviews current tool action | Medium | `codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs` |
| CF-089 (discarded-post-classification-output) | Guardian V2 Drains Generated Output After the Classification Is Known | Guardian V2 emits text after first verdict delta | Low | Set a provider output cap where supported or cancel the stream immediately after the first complete classification, while recording any usage already reported |
| CF-090 | Guardian duplicates a screenshot present in both history and retained REPL evidence | Same screenshot in history and REPL evidence | Medium | `codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs` |
| CF-093 (regular-server common-provenance) | Regular MCP Plugin Provenance Repeats on Every Child Tool | Regular plugin-backed MCP server with many tools | Medium | For regular MCP servers whose tools share server-scoped membership, render provenance once in the namespace/server description and keep connector-specific Codex Apps attribution per tool |
| CF-094 | Hidden/visible sanitized-name collision churn renames visible MCP tools | Hidden/visible sanitized collision + hidden churn | High | `codex-rs/codex-mcp/src/connection_manager/tool_catalog.rs` |
| CF-096 (false-empty-recovery) | Interrupted Deferred Search Reconstructs as Successful Empty Output | Crash after client search call before output | Medium | Reconstruct an orphaned client search as interrupted/failed (or resume it), not as a successful empty result, and preserve that status consistently in prompt history |
| CF-097 (empty-history) | Manual local compaction invokes model on empty history | Manual local compact on empty thread | Low | `codex-rs/core/src/tasks/compact.rs::CompactTask::run` |
| CF-100 (duplicate-interruption-marker) | Interrupted Fork Synthesis Can Append a Second Model-Visible Marker | Fork during interrupt marker-to-abort gap | Low | Make interrupted-boundary synthesis prefix-idempotent by detecting an existing interruption marker before appending the synthetic marker and abort event |
| CF-101 | Last-N forks drop context baselines while retaining context fragments | Last-N fork retains fragments but drops baseline | Medium | `codex-rs/core/src/agent/control/spawn.rs` |
| CF-104 | Image token heuristics diverge from active GPT-5.6 accounting | Multimodal request near context threshold | High | `codex-rs/core/src/context_manager/history.rs` |
| CF-109 | Provider ID-less output items defeat incremental continuation | Provider returns missing/empty output item ID | High | `codex-rs/core/src/client.rs::map_response_events` |
| CF-111 | Hook completion between parallel streamed calls disables continuation | Parallel calls + hook completes between siblings | High | `codex-rs/core/src/stream_events_utils.rs` & `core/src/hook_runtime.rs` |
| CF-114 | Usage recomputation counts provider-stripped passthrough metadata | Non-OpenAI normalization + metadata-heavy history | High | `codex-rs/core/src/client.rs` & `core/src/session/mod.rs` |
| CF-115 | Model switches between identical base instructions append full duplicate text | Switch between models with identical instructions | Medium | `codex-rs/core/src/context/world_state/model.rs::ModelInstructionsState::render_diff` |
| CF-116 (lost-terminal regeneration) | Missing terminal delivery discards a complete compaction item and regenerates it | Remote V2 receives compaction item but loses terminal | High | `codex-rs/core/src/compact_remote_v2.rs` |
| CF-117 | Realtime handoff duplicates user text in input and transcript delta | Realtime voice-to-text handoff | Low | `codex-rs/core/src/realtime_conversation.rs` |
| CF-120 (assistant-only transcript-tail) | Assistant-Only Realtime Transcript Tail Starts a Coding-Model Turn | Experimental realtime tail flush; assistant-only tail | Medium | `RegularTask` |
| CF-124 | Remote V2 retains prior local compaction summaries as real user messages | Remote V2 after prior local compaction | Medium | `codex-rs/core/src/compact_remote_v2.rs::build_v2_compacted_history` |
| CF-125 | Manual Remote V2 compaction samples pristine history | Manual Remote V2 compact on pristine thread | Medium | `codex-rs/core/src/compact_remote_v2_attempt.rs::run_remote_compact_v2_attempt` |

---

## Detailed Bug Findings

### CF-001: Temporary Title and Recap Workers Request Discarded Reasoning Output (Complete)

- **Status:** **Complete** — current `HEAD` no longer requests discarded reasoning summaries for temporary
  structured recap/title requests.
- **Current implementation:** Temporary structured recap/title threads are launched with
  `ThreadSource::Feature("system")` and flow through
  `codex-rs/tui/src/temporary_structured_request.rs:202` plus
  `codex-rs/tui/src/app/recap.rs:260-272` and
  `codex-rs/tui/src/app/thread_title.rs:43-70`.
- **Current fix seam:** `reasoning_summary_for_request` in `codex-rs/core/src/client.rs` suppresses summaries
  for `ThreadSource::Feature("system")` and `ThreadSource::Feature("title")`.
- **Regression evidence:** `codex-rs/core/src/client_tests.rs` asserts `temporary_structured_requests_omit_reasoning_summaries`
  and verifies recap/title paths keep `reasoning.summary = None`.

### CF-002: Goal Continuations Persist Duplicate Static Policy and Materialized Objective (Complete)

- **Status:** **Complete** — current `HEAD` retains the static goal policy and objective once per
  goal revision while automatic continuation injects only bounded turn-specific accounting state.
- **Current implementation:** `goal_context_world_state_section` in
  `codex-rs/ext/goal/src/steering.rs` publishes the static rubric and objective as a World State
  section keyed by goal ID and objective. An unchanged snapshot emits no new fragment; a missing
  retained revision is rehydrated after compaction, and an objective update creates a new revision.
- **Current continuation seam:** `GoalRuntimeHandle::continue_if_idle` injects
  `continuation_delta_steering_item`, whose `continuation_delta.md` payload contains the current
  token usage, budget, remaining tokens, and a short continuation instruction.
- **Preserved behavior:** XML escaping, goal clearing, objective revision, and pause/resume retain
  their established semantics while the static policy remains available in every context window.
- **Regression evidence:**
  `goal_context_revision_is_stable_and_rehydrates_after_compaction` in
  `codex-rs/ext/goal/tests/goal_extension_backend.rs` verifies one static revision across ten
  continuations, compaction rehydration, objective replacement, XML escaping, goal clearing, and
  pause/resume stability.

### CF-003: Memory Phase 1 Serializes Citation Markup and Raw Media Payloads as Text (Complete)

- **Status:** **Complete** — current `HEAD` sends Memory Phase 1 a sanitized, text-oriented rollout
  projection with citation syntax removed and retained image/audio payloads replaced by compact
  modality placeholders.
- **Current implementation:**
  `codex-rs/memories/write/src/phase1_projection.rs::serialize_filtered_rollout_response_items`
  projects eligible rollout items before `phase1.rs` builds the Stage 1 prompt. The projection
  sanitizes messages, agent messages, function outputs, custom-tool outputs, and compaction
  replacement history, then applies secret redaction to the serialized result.
- **Current sanitization seam:** Structured citations and legacy `<memory_citation>` blocks are
  stripped from text. Typed content items, inline image/audio data URLs, and media fields nested in
  JSON output become bounded placeholders such as `[Image: image/png]` and `[Audio: audio/wav]`.
- **Preserved behavior:** Surrounding conversational text, media type hints, and function/custom
  call relationships remain available to the extraction model.
- **Regression evidence:**
  `stage_one_input_sanitizes_citations_and_media_without_losing_call_relationships` in
  `codex-rs/memories/write/src/phase1.rs` covers assistant citations, user image/audio, function
  output media, custom JSON media, and compacted replacement history. It asserts that citation
  markers, data-URL prefixes, and every raw payload are absent from the outbound Stage 1 input
  while useful text, call IDs, and modality placeholders remain.

### CF-005: Additional Context Fingerprinting Uses Unrendered Raw Values (Complete)

- **Status:** **Complete** — current `HEAD` deduplicates additional context from the exact rendered,
  token-truncated text that becomes model input.
- **Current implementation:** `AdditionalContextStore::prepare` in
  `codex-rs/core/src/state/additional_context.rs` first renders each untrusted or application
  fragment through `AdditionalContextUserFragment` or `AdditionalContextDeveloperFragment`. It
  fingerprints the rendered `InputText`, compares the treatment and fingerprint with the committed
  snapshot, and publishes the same rendered fragment when that model-visible state changes.
- **Current rendering seam:** `codex-rs/context-fragments/src/additional_context.rs` applies
  `truncate_middle_with_token_budget` with the 1,000-token value budget before the v2 fingerprint is
  calculated. Raw tail changes outside that projection therefore retain the same snapshot identity.
- **Preserved behavior:** Changes to rendered bytes or trust treatment still publish, committed and
  restored snapshots suppress unchanged values, and an explicit clear permits the same value to be
  published again.
- **Regression evidence:** `render_equivalent_tail_changes_are_suppressed_after_truncation` and
  `changes_in_the_rendered_projection_are_published` in
  `codex-rs/core/src/state/additional_context_tests.rs` cover the cutoff invariant in both
  directions. The same test module also covers commit, restore, and explicit-clear lifecycle
  behavior.

### CF-006: Compaction Rehydration Omits Deduplication Baselines for Additional Context (Complete)

- **Status:** **Complete** — the current worktree carries the rendered additional-context
  fingerprint through compaction checkpoints and restores it during cold resume or fork.
- **Current implementation:**
  `Session::rehydrate_additional_context_for_compaction` in
  `codex-rs/core/src/session/additional_context.rs` returns the retained model-visible fragments
  together with the exact committed `AdditionalContextSnapshot`. When pre-turn or manual
  compaction uses `InitialContextInjection::DoNotInject`, `Session::replace_compacted_history` in
  `codex-rs/core/src/session/mod.rs` persists a full `WorldStateItem` whose sole section is
  `additional_context` and installs the same snapshot as the live history baseline.
- **Boundary behavior:** Mid-turn `InitialContextInjection::BeforeLastUserMessage` continues to
  persist its complete world-state baseline. Rollout reconstruction reads the
  `AdditionalContextState` section from either checkpoint shape and restores
  `AdditionalContextStore` before the next `PublishSnapshot` action.
- **Preserved behavior:** Other initial-context sections stay absent from a `DoNotInject`
  checkpoint and remain eligible for normal reinjection on the next turn. An empty
  additional-context snapshot emits no partial checkpoint.
- **Regression evidence:**
  `manual_compaction_preserves_additional_context_projection` in
  `codex-rs/core/tests/suite/additional_context_compaction.rs` covers Local, Remote V1, and Remote
  V2 compaction, each followed by cold resume and fork. All six cases assert that the persisted
  full checkpoint contains exactly `additional_context` and that the reconstructed turn contains
  exactly one application fragment and one untrusted fragment after the same snapshot is
  published again.

### CF-007: Code Mode Callbacks Route to Stale Active Turns via Shared Broker (Complete)

- **Status:** **Complete** — current `HEAD` binds each ready Code Mode cell to its originating
  session and turn, then dispatches callbacks through the worker registered for that turn.
- **Current implementation:** `CodeModeDispatchBroker` stores `CellOwner` alongside each cell gate,
  rejects callbacks after `Session::is_turn_running(owner.turn_id)` fails, and routes accepted
  callbacks through the matching per-turn worker. Notifications use the atomic
  `Session::inject_if_turn_running` path, and interruption selects only cells owned by the
  interrupted turn.
- **Preserved behavior:** Yielded cells remain available to explicit `wait` and termination calls.
  H028's conditional same-turn `StepContext` handoff remains recorded in
  `TOKEN_AUDIT_EVIDENCE.md`; it does not establish the cross-turn model-token burn claimed here.
- **Regression evidence:**
  `code_mode_background_callbacks_do_not_route_through_a_later_turn` verifies that a delayed
  nested tool and notification from turn 1 cannot execute or enter turn 2, while
  `code_mode_interrupt_terminates_active_cells_and_nested_tools` verifies turn-scoped interruption.

### CF-008: Compaction and Synchronous Guardian Request Discarded Reasoning Summaries (Complete)

- **Status:** **Complete** — compaction and synchronous Guardian requests omit the unused
  `reasoning.summary` request while retaining their selected reasoning effort.
- **Current implementation:** `ModelClient::reasoning_summary_for_request` maps summaries to
  `ReasoningSummary::None` for `CodexResponsesRequestKind::Compaction` and
  `ThreadSource::GuardianReview`. Local and remote-v2 compaction share this request builder, as do
  Guardian review sessions.
- **Preserved behavior:** Ordinary user turns, automation features, and other summary-capable
  consumers retain their configured or model-default summaries. Model capability filtering and
  reasoning-effort selection remain unchanged.
- **Regression evidence:** `reasoning_summary_policy_preserves_supported_consumers` covers the
  centralized policy and effort preservation. `previous_model_compaction_resolves_selected_settings`
  exercises the compaction request, and `guardian_denial_rejects_tool_call_with_rationale`
  exercises a summary-capable synchronous Guardian request with an explicit reasoning effort.

### CF-009: Filtered Legacy Forks Inherit Stale Parent Token Usage and Compact Prematurely (Complete)

- **Status:** **Complete** — filtered forks initialize admission accounting from their retained
  child prompt instead of the parent's last provider-reported usage.
- **Current implementation:** `keep_forked_rollout_item` drops inherited `TokenCount` events.
  `Session::record_initial_history` recomputes fork usage from the reconstructed history and the
  child's effective base instructions, while resumed sessions continue restoring recorded usage.
- **Preserved behavior:** Forked model history, reference-context reuse, rollout-budget ownership,
  and rate-limit reporting remain unchanged. Explicit post-compaction token recomputation still
  publishes its normal `TokenCount` event.
- **Regression evidence:** `filtered_legacy_fork_recomputes_usage_before_its_first_turn` gives the
  parent and child different compaction limits, seeds the parent above the child's limit, and
  verifies the child records a smaller local estimate and reaches ordinary inference on turn 1
  without a compaction request.

### CF-010: Rollout Reconstruction Re-Injects Initial Context Baseline (Complete)

- **Status:** **Complete** — current `HEAD` recognizes surviving full `WorldState` and
  `TurnContext` snapshots as proof that a replayed prefix already established canonical initial
  context, so reconstruction without a user turn boundary no longer re-emits the full context
  bundle on the next turn.
- **Current implementation:** `finalize_active_segment` in
  `codex-rs/core/src/session/rollout_reconstruction.rs` computes
  `has_surviving_context_baseline`: a segment holding both a `TurnContextItem` and a full
  `WorldState` snapshot recorded before the newest compaction boundary promotes the reference
  baseline even when the segment contains no user-message boundary.
- **Preserved behavior:** Segments without that evidence, world-state patches without a full
  snapshot, snapshots superseded by a newer compaction, and legacy compaction without
  replacement history continue to re-inject initial context as before.
- **Regression evidence:**
  `record_initial_history_restores_context_baselines_without_user_boundary` verifies a resumed
  compacted-prefix session restores its reference baseline and renders exactly one full context
  bundle, and `compacted_prefix_fork_reuses_initial_context_without_user_boundary` in
  `codex-rs/core/tests/suite/fork_thread.rs` forks from a compacted prefix and asserts the model
  request carries exactly one `<environment_context>` fragment.

### CF-012: Deferred Tool Search Retains Duplicate Schema Batches in History (Complete)

- **Status:** **Complete** — the current worktree deduplicates deferred-tool definitions in ordinary
  model history and removes schema bodies from every local, remote-v1, and remote-v2 compaction
  request while preserving the latest result that the coding model still needs to consume.
- **Current implementation:** `ToolDiscoveryState` in
  `codex-rs/core/src/context_manager/tool_discovery.rs` fingerprints individual definitions,
  preserves every call/output envelope, and retains changed schema revisions. Its rolling
  1,024-entry fingerprint window stays bounded while continuing to deduplicate recent definitions.
- **Lifecycle boundary:** One bounded client `ToolSearchOutput` is cached as pending until the next
  model-generated continuation. History rebuilds restore its full body only when the rebuilt
  history independently identifies the same call as its latest unconsumed result.
- **Compaction projection:** Local compaction and the shared remote-v1/v2 output-rewrite path clear
  every `tools` array before inference. `Session::replace_compacted_history` reinstalls the pending
  call/output pair immediately before the terminal compaction summary, persists that same
  replacement, and leaves installed-history traces aligned with the live context.
- **P0 model-visible size review:** The reinstated output can approach the existing 32 KiB search
  ceiling and therefore exceed 1,000 tokens. Exactly one such result is retained; the tools body
  is 8,192 tokens at the repository's byte-based estimator, its fixed envelope keeps the item
  below the 10K-token ceiling, and the first coding-model continuation clears it.
- **Preserved behavior:** Distinct definitions and revisions survive; duplicate result envelopes,
  canonical IDs, statuses, execution modes, and harness metadata remain intact. Projection is
  idempotent, so retries and model fallback do not multiply schema content.
- **Regression evidence:**
  `mid_turn_compaction_omits_search_schemas_and_restores_the_live_result` in
  `codex-rs/core/tests/suite/tool_search_compaction.rs` covers Local, Remote V1, and Remote V2 and
  proves each compaction request sees an empty schema body while the immediate coding-model
  continuation sees exactly one full matching call/output pair. Discovery-state tests cover
  consumption and rebuild behavior; the existing repeated-search and remote-compaction tests
  continue to prove one retained schema and schema-free compaction projections.

### CF-014: MCP Resource Listings Repeat Server Identity per Descriptor (Complete)

- **Status:** **Complete** — explicit single-server resource/template listings emit one top-level
  `server` field and omit per-descriptor server identity.
- **Current implementation:** `ListResourcesPayload::from_single_server` and
  `ListResourceTemplatesPayload::from_single_server` in
  `codex-rs/core/src/tools/handlers/mcp_resource.rs` wrap every descriptor with
  `ResourceListingEntry::without_server`, so the group envelope carries the server identity once.
  Flattened all-server listings keep per-descriptor `with_server` ownership for unambiguous
  attribution, and `next_cursor` passthrough is preserved on the single-server path.
- **Preserved behavior:** Flattened all-server listings remain sorted by server with ownership on
  each descriptor, and single-server pagination continues through the existing cursor path.
- **Regression evidence:** `list_resources_payload_from_single_server_emits_server_once_and_copies_cursor`
  and `list_resource_templates_payload_from_single_server_omits_child_server` assert the
  serialized envelope; `explicit_server_resource_listings_emit_server_once` in
  `codex-rs/core/tests/suite/mcp_resource.rs` proves the server appears once per listing across
  both resources and templates in a live model round-trip.

### CF-015: Generic MCP Resource Reads Flatten Binary Content into JSON/Base64 Text (Complete)

- **Status:** **Complete** — generic MCP resource reads project text, image, and binary content
  into typed `FunctionCallOutputContentItem` objects rather than JSON-stringified envelopes.
- **Current implementation:** `project_read_resource_output` in
  `codex-rs/core/src/tools/handlers/mcp_resource/read_mcp_resource.rs` renders a compact
  identity header (URI, server, media type) plus one typed content item per `ResourceContents`:
  text stays text, supported image media become `InputImage` data URLs, and unhandled binaries
  are replaced with a bounded `[binary payload omitted: N base64 characters]` notice instead of
  raw base64.
- **Preserved behavior:** Server and URI metadata remain available in the compact header, and
  the output continues through the shared per-output truncation policy.
- **Regression evidence:** `read_resource_projection_preserves_text_with_compact_identity_header`,
  `read_resource_projection_emits_image_as_typed_content`, and
  `read_resource_projection_omits_raw_unhandled_binary_payload` cover the three projections;
  `read_mcp_image_resource_emits_typed_image_content` in
  `codex-rs/core/tests/suite/mcp_resource.rs` proves a live read produces a typed image item
  with no raw base64 in any text segment.

### CF-016: Normal MCP Results JSON-Stringify Embedded Resources and Media (Complete)

- **Status:** **Complete** — embedded MCP resource and media blocks project into typed content
  items instead of JSON-stringified text.
- **Current implementation:** `convert_mcp_content_to_items` in
  `codex-rs/protocol/src/models/mcp_content.rs` maps embedded images and audio directly to
  typed `FunctionCallOutputContentItem::InputImage`/`InputAudio` data URLs (building the data
  URL prefix when absent), preserves embedded text resources with URI and text, and replaces
  unhandled blobs with a compact modality notice instead of raw base64.
- **Preserved behavior:** Ordinary text and structured MCP content keep their existing
  projection; `structuredContent` and Code Mode paths remain unchanged.
- **Regression evidence:** `embedded_image_resource_becomes_typed_image_content`,
  `embedded_text_resource_preserves_uri_and_text`, and `embedded_unhandled_blob_omits_raw_base64`
  in `codex-rs/protocol/src/models/mcp_content_tests.rs` cover the typed projections, and
  `embedded_mcp_image_resource_emits_typed_image_content` in
  `codex-rs/core/tests/suite/mcp_resource.rs` proves a live `CallToolResult` with an embedded
  image resource produces a typed image item with no base64 in any text segment.

### CF-017: Request-Fit Overestimation Triggers Avoidable Compaction (Complete)

- **Status:** **Complete** — pre-turn admission accounts for the request-scoped normalization
  delta instead of raw history, so media or items that normalization will strip no longer
  trigger avoidable compaction.
- **Current implementation:** `Session::get_model_visible_token_usage` in
  `codex-rs/core/src/session/mod.rs` adds `history.model_visible_token_delta` — computed against
  the active model's input modalities and truncation policy — to the authoritative server usage
  before `context_window_token_status` decides whether
  `run_pre_sampling_compact` launches compaction.
- **Preserved behavior:** Server-reported usage remains the baseline so provider-counted
  instructions and tools are retained; only the media/history items normalization replaces or
  discards are deducted.
- **Regression evidence:** `normalized_history_that_fits_does_not_trigger_compaction` in
  `codex-rs/core/tests/suite/request_fit.rs` seeds a multimodal-history turn over a
  text-only-model's compact limit, switches to the text model, and asserts the follow-up request
  contains no `compaction_trigger` input while the normalized image is absent from the
  transmitted request.

### CF-018: Local Compaction Resubmits Full Prompt Iteratively and Retains Failed Output (Complete)

- **Status:** **Complete** — local compaction reduces whole message groups in one bulk step per
  rejection and stages generated output in memory, committing to history only on terminal
  success.
- **Current implementation:** `run_compact_task_inner_impl` in
  `codex-rs/core/src/compact.rs` drives a `LocalCompactionPlan` whose
  `reduce_after_context_error` removes complete turn groups in bulk to a safe budget, and
  `drain_to_completed` stages partial streams so an `OutputItemDone` followed by stream failure
  never writes to live or rollout history.
- **Preserved behavior:** Interrupted/aborted and session-budget errors continue to surface
  immediately; a bounded retry count still applies to context-window rejections.
- **Regression evidence:** `context_rejection_reduces_multiple_complete_turn_groups_once` and
  the replacement-budget tests in `codex-rs/core/src/compact/local_plan_tests.rs` prove bulk
  reduction and a bounded final replacement;
  `failed_local_compaction_output_is_absent_after_resume` in
  `codex-rs/core/tests/suite/local_compaction.rs` proves a failed compaction stream leaves no
  persisted or resumed model-visible text.

### CF-019: Final Tool Output Can Exceed Its Advertised Per-Output Cap (Complete)

- **Status:** **Complete** — `truncate_function_output_payload` is the final authority over the
  serialized function/custom-tool output, and the final payload stays within the configured cap.
- **Current implementation:** `codex-rs/core/src/context_manager/function_output.rs` measures
  the complete serialized payload with an o200k tokenizer, charges separator and structural
  framing for every content item (including media and encrypted blocks via
  `non_text_token_cost`), drops the old `* 1.2` expansion, and re-finalizes after
  request-scoped unsupported-audio projection through `for_prompt_with_policy`.
- **Preserved behavior:** Small payloads pass through unchanged, byte policies charge the
  complete serialized media item, and omission markers participate in the same budget.
- **Regression evidence:** `token_dense_text_fits_the_nominal_output_limit`,
  `high_cardinality_text_items_include_structural_cost`,
  `empty_encrypted_items_have_nonzero_structural_cost`,
  `zero_duration_audio_items_have_nonzero_structural_cost`,
  `omission_marker_participates_in_the_same_budget`, and
  `unsupported_audio_projection_is_finalized_again` in
  `codex-rs/core/src/context_manager/function_output_tests.rs` cover every required case, and
  `high_cardinality_mcp_output_stays_within_configured_cap` in
  `codex-rs/core/tests/suite/final_payload_cap.rs` proves a 12K-item MCP output serializes to
  at most 10K real o200k tokens in a live round-trip.

### CF-020: Tool Output Lifecycle Re-Expands on Resume and Retains Stale Notices

- **Sources & Facet:** #25, H064, H080 (stale-notice/re-expansion facets)
- **Trigger, Mechanism & Root Cause:** `prepare_conversation_items_for_history` and `record_prepared_conversation_items` (`codex-rs/core/src/session/mod.rs:3065-3220`) generate image resize notices before subsequent retention logic drops images. On cold resume, truncated tool outputs are re-evaluated against the current model's policy, re-expanding previously truncated output to a larger cap.
- **Demonstrated Token Impact:** Stale image notices remain in prompt context for images that no longer exist, and resuming on a larger model expands historical tool outputs by thousands of tokens.
- **Remediation Seam:** Apply irreversible retention once at the durable boundary, generate resize notices only from retained images, and persist the exact model-independent retained payload so resume never re-expands historical outputs.
- **Required Verification:** Resume a session with small-policy truncated tool outputs under a model with larger policy; verify that old tool outputs remain at their historical truncated size.

### CF-021: Remote V2 Undercharges Audio while Normal History Overcharges Durationless Audio

- **Sources & Facet:** #16, H016, H106 (audio-accounting facets)
- **Trigger, Mechanism & Root Cause:** Remote V2 compaction (`codex-rs/core/src/compact_remote_v2.rs:601-733`) budgets retained audio items as 0 tokens in its retention pass. Conversely, `estimate_audio_token_count` (`codex-rs/utils/audio/src/lib.rs:149-186`) falls back to raw base64 string size when container duration is missing, overestimating valid audio by an order of magnitude.
- **Demonstrated Token Impact:** Remote V2 retains audio without deducting budget, causing post-compaction overflow, while normal history triggers premature compaction due to base64 overestimation.
- **Remediation Seam:** Use duration-derived token estimation with a bounded frame/packet fallback for durationless audio across both normal history budgeting and Remote V2 retention.
- **Required Verification:** Feed valid audio lacking header duration into history; verify estimation uses bounded packet fallback rather than base64 size. In Remote V2, verify retained audio is charged accurately against the retention budget.

### CF-022: Temporary Structured Turn Timeout Fails to Cancel Active Model Inference

- **Sources & Facet:** #5 (stale cancellation facet)
- **Trigger, Mechanism & Root Cause:** `run_temporary_structured_turn` (`codex-rs/tui/src/temporary_structured_request.rs:240-295`) calls `thread/unsubscribe` on timeout or discard. However, `thread/unsubscribe` only detaches the client listener; it does not issue `turn/interrupt` to the app-server, so the background model turn continues executing to completion.
- **Demonstrated Token Impact:** Discarded title and recap generations continue running on the provider and consuming output tokens after the user interface has given up.
- **Remediation Seam:** On timeout or cancellation in `run_temporary_structured_turn`, send an explicit `turn/interrupt` request to the active turn before unsubscribing.
- **Required Verification:** Trigger a temporary structured request with a simulated client timeout; verify that a `turn/interrupt` RPC is transmitted and the active model turn is aborted.

### CF-025: Queued Turn Dispatches Before Durable Deletion Commits (Complete)

- **Status:** **Complete** — queued items move through a durable claim state machine
  (`Pending -> Claimed { turn_id } -> Completed`) so a crash or deletion failure can no longer
  re-dispatch a turn that already started.
- **Current implementation:** `QueuedItemService::start` in
  `codex-rs/ext/queue/src/service.rs` marks the row `Claimed` with the turn ID before
  dispatching the turn, releases the claim on failure, and `QueuedItemsRuntime` in
  `codex-rs/state/src/runtime/queued_items.rs` reconciles claimed rows on startup.
- **Preserved behavior:** FIFO ordering, edits/reordering, pagination, and per-thread queue
  limits are unchanged.
- **Regression evidence:** `cf_025_claimed_items_are_hidden_until_released_or_completed` and
  `cf_025_startup_reconciliation_completes_only_claimed_items` in
  `codex-rs/state/src/runtime/queued_items_tests.rs` cover the claim lifecycle and startup
  reconciliation.

### CF-027: Cold Resume Resets Shared Rollout Budget Ledger to Zero

- **Sources & Facet:** H020
- **Trigger, Mechanism & Root Cause:** `AgentControl` initializes an in-memory `RolloutBudget` tracking consumed tokens (`codex-rs/core/src/rollout_budget.rs:35-64`). On cold session resume, `thread_manager.rs:1076-1110` constructs a fresh `RolloutBudget` initialized to 0 because consumed units are not persisted in rollout state.
- **Demonstrated Token Impact:** Reopens the configured rollout token budget upon every restart or cold resume, bypassing user-configured token limits.
- **Remediation Seam:** Persist cumulative rollout budget consumption in session checkpoint state, and restore the exact consumed token count upon cold resume.
- **Required Verification:** Consume 80% of a rollout token budget, perform cold resume; verify that remaining budget is 20%, not reset to 100%.

### CF-028: Same-Window Resume Re-Arms One-Shot Reminder Delivery State

- **Sources & Facet:** H021
- **Trigger, Mechanism & Root Cause:** Time and token reminders maintain one-shot delivery state in `SessionState` (`codex-rs/core/src/state/session.rs:66-79`). On session resume, reminder state is re-initialized, causing reminders already present in historical messages to be generated and appended again in the restored window.
- **Demonstrated Token Impact:** Appends duplicate reminder messages into context after every resume in the same active window.
- **Remediation Seam:** Inspect restored history during `apply_rollout_reconstruction` to restore the latest reminder delivery timestamps and prevent duplicate injection in the same window.
- **Required Verification:** Trigger a reminder, resume the session in the same time window; verify that no duplicate reminder is injected on turn 1.

### CF-030: Goal Continuation Ignores Interrupted Idle Cause and Resumes Active Turn (Complete)

- **Status:** **Complete** — automatic goal continuation is gated on the idle cause, so a direct
  interrupt path can no longer immediately relaunch an active goal.
- **Current implementation:** `GoalExtension::on_thread_idle` in
  `codex-rs/ext/goal/src/extension.rs` matches `ThreadIdleCause` and returns early for
  `Interrupted` and `Failed`, continuing only on `Completed`.
- **Preserved behavior:** TUI flows that pause the goal before interrupting keep their existing
  pause/resume semantics, and ordinary completed-turn continuation is unchanged.
- **Regression evidence:** `cf_030_interrupted_goal_turn_does_not_continue` in
  `codex-rs/app-server/tests/suite/v2/goal_context.rs` interrupts an active goal turn over the
  app-server path (no TUI pause), asserts no fourth `response.create` continuation request is
  issued, and verifies the goal remains persisted as Active.

### CF-033: Goal Tool Responses Echo Full Objective and Derived State

- **Sources & Facet:** H054 (create/update projection facet)
- **Trigger, Mechanism & Root Cause:** `GoalToolExecutor` (`codex-rs/ext/goal/src/tool.rs:64-66, 186-222`) serializes the full `ThreadGoal` struct—including full objective text, schema, IDs, and timestamps—into the tool response payload on every create or update call.
- **Demonstrated Token Impact:** Duplicates the entire objective text in the tool output immediately after the model has already supplied it in the tool call arguments.
- **Remediation Seam:** Return a compact acknowledgment DTO (e.g. `GoalOperationResult { goal_id, status }`) rather than echoing the full goal objective and state.
- **Required Verification:** Create and update a goal via tool calls; assert tool response payloads contain only status and ID, omitting the full objective text.

### CF-034: Swallowed Goal Accounting Persistence Error Launches Duplicate Turn

- **Sources & Facet:** H055 (swallowed-accounting-error facet)
- **Trigger, Mechanism & Root Cause:** When `account_active_goal_progress` (`codex-rs/ext/goal/src/runtime.rs:390-417`) fails to persist goal progress to storage, the error is logged and swallowed. `on_thread_idle` continues to see the goal as active and unprogressed, immediately launching another continuation turn.
- **Demonstrated Token Impact:** Spins in an automatic continuation loop, executing repeated model turns when storage is failing.
- **Remediation Seam:** Propagate accounting persistence errors and stop automatic goal continuation when accounting cannot be persisted.
- **Required Verification:** Inject a database error into goal accounting persistence; verify that automatic continuation halts and reports an error.

### CF-038: Phase 1 Memory Leases Can Expire Before Queued Jobs Start

- **Sources & Facet:** H101 (expired-lease duplicate-sampling facet)
- **Trigger, Mechanism & Root Cause:** Phase 1 claims a batch of extraction jobs on startup with a 1-hour lease (`codex-rs/memories/write/src/phase1.rs:149-220`). If execution queueing delays processing past 1 hour, another worker can re-claim the job, causing duplicate extraction.
- **Demonstrated Token Impact:** With `max_rollouts_per_startup >= 17` and sufficiently long samples, a later startup can reclaim a queued or still-running job while the original future proceeds to the model without revalidating ownership, producing two live extraction requests for one rollout.
- **Remediation Seam:** Acquire the lease immediately before sampling, heartbeat it while queued/running, and revalidate the ownership token before the model request in `memories/write/src/phase1.rs`.
- **Required Verification:** Use a non-default batch above the eight-wide concurrency limit and an expired lease; assert only the current owner reaches `stream_stage_one_prompt`.

### CF-044: Exhausted Memory Consolidation Retries Continue Claiming Workers (Complete)

- **Status:** **Complete** — exhausted Phase 2 consolidation jobs stop being claimed and are
  marked permanently failed once retries reach zero.
- **Current implementation:** `try_claim_global_phase2_job` in
  `codex-rs/state/src/runtime/memories.rs` filters the claim query with
  `AND retry_remaining > 0` and its completion path marks jobs terminally failed when the
  retry budget is exhausted.
- **Preserved behavior:** Fresh jobs, backoff-window gating, lease takeover, and success
  cooldown behavior are unchanged.
- **Regression evidence:** `cf_044_phase2_global_lock_stops_after_retry_budget_is_exhausted`
  in `codex-rs/state/src/runtime/memories.rs` proves an exhausted job is neither claimed nor
  re-launched by subsequent consolidation runs.

### CF-045: Memory Phase 1 Reuploads Discovery Schemas Retired by Compaction

- **Sources & Facet:** H042 (checkpoint-retired discovery-schema facet)
- **Trigger, Mechanism & Root Cause:** Phase 1 extraction processes raw rollout events (`codex-rs/memories/write/src/phase1.rs:289-313`), including tool search schemas that were later compacted away in active history.
- **Demonstrated Token Impact:** Each historical tool-search result can contribute up to 32 KiB of schemas to a fresh Phase 1 model request even after the active conversation checkpoint removed it. Multiple searches accumulate without an aggregate historical-schema filter.
- **Remediation Seam:** Build the Phase 1 projection from reconstructed active history, or at minimum omit pre-checkpoint `ToolSearchCall` and `ToolSearchOutput` records while preserving semantically necessary tool evidence.
- **Required Verification:** Compact a rollout containing multiple discovery results, then run Phase 1; assert checkpoint-retired schemas are absent while surviving active evidence remains.

### CF-046: MCP Resource Handlers Bypass Memory External-Context Suppression (Complete)

- **Status:** **Complete** — MCP resource tool outputs are marked as external context, so the
  memory pollution guard suppresses extraction on threads containing third-party resource
  dumps.
- **Current implementation:** `run_resource_operation` in
  `codex-rs/core/src/tools/handlers/mcp_resource.rs` returns
  `output.with_external_context()`, flowing through the registry's external-context guard.
- **Preserved behavior:** Ordinary MCP tool-call outputs and non-resource tools keep their
  existing external-context behavior.
- **Regression evidence:** `cf_046_mcp_resource_marks_thread_memory_mode_polluted` in
  `codex-rs/core/tests/suite/mcp_resource.rs` performs a live `read_mcp_resource` call and
  asserts the thread's memory mode is recorded as polluted, proving extraction suppression.

### CF-049: Parsed Memory Citations Remain in Ordinary Model History

- **Sources & Facet:** H103 (ordinary model-history / prompt-projection facet)
- **Trigger, Mechanism & Root Cause:** Raw assistant citations are preserved in stored conversation items and stripped only on client-facing events (`codex-rs/core/src/stream_events_utils.rs:76-127`).
- **Demonstrated Token Impact:** The hidden citation block remains part of logical model context, is replayed on full-create/reconnect paths, is consumed by compaction, and is serialized again by later Phase 1 extraction. A checked-in two-entry example is about 85 heuristic tokens before JSON escaping.
- **Remediation Seam:** Preserve the raw response for rollout provenance, but strip hidden citation syntax in the prompt, compaction, and memory-extraction projections using the already parsed structured citation metadata.
- **Required Verification:** Follow a citation-bearing assistant response through later prompt construction, compaction, and Phase 1; assert raw storage retains provenance while no model-facing projection contains the hidden citation block.

### CF-052: Rollback Resurrects Asynchronous Hook Context from Deleted Turns

- **Sources & Facet:** H006
- **Trigger, Mechanism & Root Cause:** When `thread_rollback` is invoked (`codex-rs/core/src/session/handlers.rs:244-360`), active asynchronous hook tasks from the rolled-back turn are not cancelled. When those tasks eventually complete, `drain_async_hook_results` injects their results into the current turn without verifying if their originating `turn_id` was rolled back.
- **Demonstrated Token Impact:** Injects stale, irrelevant hook context from deleted turns into new model turns, wasting context window space.
- **Remediation Seam:** Track originating `turn_id` on async hook handles and cancel pending hook tasks upon rollback; discard results if their turn was removed.
- **Required Verification:** Trigger a slow async hook, immediately roll back the turn, and start a new turn; verify that the completed hook result is discarded.

### CF-054: Environment World State Emits Complete Bundle on Any Field Change

- **Sources & Facet:** #28 (environment delta facet)
- **Trigger, Mechanism & Root Cause:** `EnvironmentsState::render_diff` (`codex-rs/core/src/context/world_state/environment.rs:103-151, 246-265`) detects any single environment field change (e.g. current directory or timezone) and re-emits the entire environment block including all unchanged paths and settings.
- **Demonstrated Token Impact:** Appends redundant environment text on every turn where minor metadata changes.
- **Remediation Seam:** Implement field-level diffing in `EnvironmentsState` so that only changed environment variables or paths are emitted in the update fragment.
- **Required Verification:** Change only the working directory; verify the emitted World State diff contains only the updated directory field.

### CF-056: Guidance Sections Re-Emit Stale Instructions Across Silent Re-Enable

- **Sources & Facet:** H086
- **Trigger, Mechanism & Root Cause:** When guidance sections (apps, plugins, environments) are disabled, no model-visible tombstone is emitted. When re-enabled, `WorldState::render_history_diff` (`codex-rs/core/src/context/world_state/mod.rs:38-49`) compares against active state and re-emits the entire instruction set into context.
- **Demonstrated Token Impact:** Duplicates large blocks of static system guidance in conversational history.
- **Remediation Seam:** Track history-aware presence of guidance fragments across transitions to avoid re-emitting guidance that remains in active history.
- **Required Verification:** Toggle a guidance section off and on; verify that identical instructions already present in context are not re-appended.

### CF-060: Skill Catalog Updates Append Full Current Body Instead of Deltas

- **Sources & Facet:** H094
- **Trigger, Mechanism & Root Cause:** `skills_world_state_section` (`codex-rs/ext/skills/src/world_state.rs:77-118`) detects a skill catalog modification and appends the complete current catalog text into history, duplicating all unchanged skill definitions.
- **Demonstrated Token Impact:** Appends thousands of tokens of redundant skill descriptions on every catalog change.
- **Remediation Seam:** Render skill catalog changes as entry-level deltas (added, removed, modified skills) rather than re-emitting the entire catalog.
- **Required Verification:** Add one new skill to an existing 20-skill catalog; assert that only the added skill definition is emitted in the update.

### CF-063: Multi-Authority Skill Catalogs Repeat Shareable Generic Guidance

- **Sources & Facet:** H096 (repeated-generic-guidance facet)
- **Trigger, Mechanism & Root Cause:** `render_combined_available_skills` (`codex-rs/ext/skills/src/render.rs:540-655`) repeats exact/common trigger, coordination, context-hygiene, and fallback guidance across multiple authority fragments. Authority-specific trust/provenance wording is not claimed to be duplicate and remains a separate design question.
- **Demonstrated Token Impact:** The exact common subset is duplicated in one model-visible catalog. Broader 1.5K-1.8K “shareable” estimates are optimization bounds, not established duplicate bytes.
- **Remediation Seam:** Factor common skill usage and formatting instructions into a single top-level header, rendering only specific skill items per authority.
- **Required Verification:** Render a multi-authority skill catalog; verify that common usage guidance appears once at the top level.

### CF-067: Unified Exec Retains Orphaned Process Handles Across Interrupt and Resume

- **Sources & Facet:** H009
- **Trigger, Mechanism & Root Cause:** `ProcessManager` (`codex-rs/core/src/unified_exec/process_manager.rs:540-563`) destroys in-memory process handles on shutdown or interrupt, but reconstructed rollout history retains the old process IDs. When the model attempts to poll or write to the handle on resume, the call fails with `ProcessNotFound`, forcing an unnecessary error recovery round.
- **Demonstrated Token Impact:** Forces a failed tool execution and a corrective model inference turn on resume.
- **Remediation Seam:** During rollout reconstruction, mark orphaned Unified Exec process handles as terminated with an explicit boundary record.
- **Required Verification:** Start a background process, interrupt and resume the session; verify that process handle is marked dead and does not induce failed tool calls.

### CF-069: Non-TTY Unified Exec Sessions Advertise Unsupported Stdin Interaction

- **Sources & Facet:** H011
- **Trigger, Mechanism & Root Cause:** `default_tty` defaults to false, yet the tool specification in `shell_spec.rs:91-145` advertises `write_stdin` as fully supported. Calling `write_stdin` on a non-TTY process returns `StdinClosed`, causing an avoidable tool error and corrective model follow-up.
- **Demonstrated Token Impact:** Causes failed tool executions and corrective model turns for non-interactive commands.
- **Remediation Seam:** Accurately reflect TTY/stdin capabilities in the tool spec or automatically allocate a pseudo-terminal when interactive stdin is requested.
- **Required Verification:** Call `write_stdin` on a non-TTY command; verify that the tool schema or execution handles stdin correctly without error.

### CF-071: Code Mode Resume Restores Dead Cell IDs Forcing Failed Wait Continuations

- **Sources & Facet:** H027
- **Trigger, Mechanism & Root Cause:** On cold resume, `CodeModeService` creates an empty in-memory cell table while rollout history retains yielded cell IDs (`codex-rs/core/src/tools/code_mode/mod.rs:69-90`). When the resumed model invokes `wait(cell_id)`, the gRPC runtime rejects the dead cell ID, requiring an extra model turn to recover.
- **Demonstrated Token Impact:** Causes guaranteed failed wait calls and extra recovery inferences upon resume.
- **Remediation Seam:** Reconcile active Code Mode cell IDs during rollout reconstruction, marking unrecoverable cells as terminated before model prompt assembly.
- **Required Verification:** Yield a Code Mode cell, save and resume the session; verify that subsequent wait calls handle the restored state cleanly.

### CF-073: Fatal Asynchronous Tool Errors are Swallowed and Trigger Empty Follow-Ups

- **Sources & Facet:** H072
- **Trigger, Mechanism & Root Cause:** In release builds, `drain_in_flight` (`codex-rs/core/src/session/turn.rs:2155-2177`, `core/src/tools/parallel.rs:80-85`) logs fatal async tool errors but returns `Ok`, leaving `needs_follow_up` set. The runtime then launches an unnecessary model request with synthetic empty tool outputs.
- **Demonstrated Token Impact:** Triggers an avoidable model inference turn on fatal tool failures.
- **Remediation Seam:** Propagate fatal tool errors immediately from `drain_in_flight`, failing the turn without issuing a follow-up model request.
- **Required Verification:** Inject a fatal error into an async tool; verify that the turn halts immediately and does not issue a follow-up inference.

### CF-076: Reused V1 Subagents Fail to Rearm Completion Watchers

- **Sources & Facet:** H023
- **Trigger, Mechanism & Root Cause:** The V1 completion watcher is attached only at subagent spawn or resume (`codex-rs/core/src/agent/control.rs:184-210`). When sending subsequent inputs to a reused subagent via `send_input`, the watcher is not re-armed, forcing the parent model to poll status repeatedly.
- **Demonstrated Token Impact:** Forces repeated model polling turns for reused subagent results.
- **Remediation Seam:** Re-arm the completion watcher in `send_input` whenever a new turn is submitted to an existing subagent.
- **Required Verification:** Submit multiple turns to a reusable subagent; verify that completion notifications are received automatically without polling.

### CF-077: Rendered Subagent Completion Items Can Exceed Their 1K Cap (Complete)

- **Status:** **Complete** — the 1K model-visible cap is enforced across the complete rendered
  completion envelope, including escaped payload, markers, and agent-path metadata.
- **Current implementation:** `bounded_completion_fragment` in
  `codex-rs/core/src/session_prefix.rs` renders the full envelope (markers plus body) first,
  measures it with the shared token estimator, and iteratively reduces the body budget until
  the rendered fragment fits `COMPLETION_MESSAGE_MAX_TOKENS`;
  `bounded_completion_status` applies the same reserve to raw completion payloads.
- **Preserved behavior:** Small completions render unchanged, and V1
  `<subagent_notification>` markers plus V2 inter-agent messages keep their existing envelope
  shapes.
- **Regression evidence:** `escaped_completion_payloads_and_long_paths_stay_within_cap` in
  `codex-rs/core/src/session_prefix_tests.rs` drives quote-heavy, newline-heavy, and NUL-heavy
  payloads plus a 3K-character agent path through both V1 and V2 envelopes and asserts each
  rendered fragment stays within the 1K cap; the error/completion and notification tests cover
  the ordinary paths.

### CF-078: Nonterminal Subagent Errors are Published as Terminal Failures

- **Sources & Facet:** H025
- **Trigger, Mechanism & Root Cause:** `agent_status_from_event` (`codex-rs/core/src/agent/status.rs:6-21`) treats all `ErrorEvent` occurrences as terminal subagent failures, even when the error is non-fatal and the subagent continues running, leading to conflicting results and unnecessary retries.
- **Demonstrated Token Impact:** Triggers false-failure handling and redundant agent launches while the original agent is still running.
- **Remediation Seam:** Check `ErrorEvent::is_terminal` before transitioning agent status to `Failed`.
- **Required Verification:** Emit a recoverable error event in a subagent; verify that agent status remains running until terminal completion.

### CF-082: Inline Review Findings are Persisted Twice in Parent History (Complete)

- **Status:** **Complete** — current `HEAD` records the review findings once, in the user-action
  envelope; the UI-facing AgentMessage turn item is emitted without a second history record.
- **Current implementation:** `exit_review_mode` in
  `codex-rs/core/src/tasks/review.rs` records the `<user_action>` envelope as the single
  model-visible copy, then emits the `ExitedReviewMode` and `AgentMessage` turn items directly.
  The AgentMessage item preserves the client-facing findings text while
  `record_conversation_items` never sees an assistant message, so prompt construction, compaction
  projections, and Phase 1 extraction all observe exactly one copy of the findings.
- **Preserved behavior:** Event ordering (ExitedReviewMode lifecycle, then AgentMessage lifecycle),
  the interrupted-path envelope, rollout persistence via `ensure_rollout_materialized`, and the
  v2 client projection that renders display text from the `ExitedReviewMode` item payload remain
  unchanged. Resume reconstruction reads only persisted ResponseItems, so the removed duplicate
  cannot reappear on cold resume.
- **Regression evidence:**
  `review_exit_records_findings_once_in_parent_history` in `codex-rs/core/tests/suite/review.rs`
  runs a structured review followed by a parent turn and asserts the finding body and explanation
  each appear exactly once in the outbound request input.
  `review_op_emits_lifecycle_and_review_output` now asserts the assistant duplicate is absent from
  the rollout while the user-action envelope retains the findings.

### CF-083: Reusable Guardian Delta Turns Re-Emit Root Evidence and Advance on Parse Failure

- **Sources & Facet:** H012
- **Trigger, Mechanism & Root Cause:** In reusable Guardian sessions (`codex-rs/core/src/guardian/prompt.rs:142-178`), the delta cursor only tracks transcript items, while root authorization and environment context are re-appended outside the delta branch on every turn. Furthermore, if parsing fails, the cursor advances anyway, causing missed deltas.
- **Demonstrated Token Impact:** Repeats thousands of tokens of root evidence across every reviewed tool turn.
- **Remediation Seam:** Maintain an atomic cursor that tracks both transcript and root evidence, ensuring root context is not re-appended on delta turns.
- **Required Verification:** Execute multiple Guardian review passes in a reusable session; verify root evidence is sent once and subsequent turns contain only new tool deltas.

### CF-086: Synchronous Guardian Supplies the Output Contract Twice (Complete)

- **Status:** **Complete** — the synchronous Guardian output contract is defined once in
  `text.format`, with behavioral guidance only in the instructions.
- **Current implementation:** `guardian_output_schema()` in
  `codex-rs/core/src/guardian/prompt.rs` is the sole carrier of the property/type/enum
  contract, and `guardian_output_contract_prompt()` supplies only read-only-investigation
  guidance plus the low-risk shortcut without restating any schema field.
- **Preserved behavior:** Guardian V2 remains on its separate classifier path and is excluded;
  the schema's required-fields and enum semantics are unchanged.
- **Regression evidence:** `guardian_prompt_leaves_output_shape_to_structured_schema` asserts
  no schema field name appears in the policy prompt, and the synchronous review request tests
  assert each field, type, and enum set appears exactly in `text/format/schema`.

### CF-088: Guardian V2 Classifier Prompt Duplicates Current Tool Action in Transcript

- **Sources & Facet:** H013
- **Trigger, Mechanism & Root Cause:** In Guardian V2 (`codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs:210-226`), the tool call currently under review is appended to the transcript history before review, and then rendered a second time as the planned action payload in the classifier prompt.
- **Demonstrated Token Impact:** Duplicates the tool arguments and metadata in every safety classification request.
- **Remediation Seam:** Filter out the reviewed `call_id` from the transcript history when assembling the classifier prompt.
- **Required Verification:** Run Guardian V2 on a tool call; verify that the tool call appears only in the planned-action section and not duplicated in the transcript.

### CF-089: Guardian V2 Drains Generated Output After the Classification Is Known

- **Sources & Facet:** H015 (discarded-post-classification-output facet)
- **Trigger, Mechanism & Root Cause:** On the first nonempty `OutputTextDelta`, `sampler.rs:653-686` returns the classification immediately but moves the receiver into a background task that drains all later events until `Completed`. The trigger requires Guardian V2, a provider/model that emits additional text after the requested classification token, and ordinary metered routing (`free_guardian = false`).
- **Demonstrated Token Impact:** After the first nonempty classification delta, later provider output cannot change the verdict and is consumed only by a background drain for connection reuse/accounting. With ordinary `/responses` routing (`free_guardian = false`), those later output tokens remain metered and unused.
- **Remediation Seam:** Set a provider output cap where supported or cancel the stream immediately after the first complete classification, while recording any usage already reported.
- **Required Verification:** Have the provider emit a valid first classification followed by contradictory or verbose deltas; assert generation is cancelled/capped and no later text is consumed.

### CF-090: Guardian Duplicate Screenshot Ingestion from History and REPL State

- **Sources & Facet:** H108
- **Trigger, Mechanism & Root Cause:** `TranscriptConfig::images` (`codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs:83-156`) independently collects conversation-history images and retained Node REPL evidence. The duplicate requires the same screenshot bytes to be explicitly present in both sources; no cross-source content fingerprint removes the second copy.
- **Demonstrated Token Impact:** Sends identical high-resolution screenshots twice in the same multimodal Guardian evaluation request.
- **Remediation Seam:** Deduplicate image payloads by content hash in `TranscriptConfig::images` before constructing the classifier prompt.
- **Required Verification:** Submit a turn containing both a history image and a REPL screenshot with identical bytes; assert only one image is included in the classifier request.

### CF-092: MCP Stale Tool Definitions Persist Across Catalog Changes and Recovery (Complete)

- **Status:** **Complete** — the current worktree refreshes the frozen model-visible MCP binding
  after `notifications/tools/list_changed` and after session-expiry recovery.
- **Current implementation:** `codex-rs/rmcp-client` advances a shared tool-list generation after
  initialization, recovery, and list-change notifications. `codex-rs/codex-mcp` compares that
  generation at the stable binding-cache boundary, relists through a single-flight semaphore, and
  advances the catalog revision before publishing the replacement binding.
- **Preserved behavior:** Pending startup catalogs remain nonblocking, while ready Codex Apps
  bindings continue to use the exact connection-local tool list rather than a shared startup cache.
- **Regression evidence:**
  `connection_manager::tests::tool_list_changed_refreshes_the_model_visible_binding` verifies that
  the old tool disappears from the rebuilt binding, and
  `streamable_http_404_session_expiry_recovers_and_retries_once` verifies that recovery advances
  the transport generation.

### CF-093: Regular MCP Plugin Provenance Repeats on Every Child Tool

- **Sources & Facet:** H033 (regular MCP server/common-membership facet)
- **Trigger, Mechanism & Root Cause:** Plugin provenance sentences are appended to every individual tool description in an MCP namespace (`codex-rs/codex-mcp/src/rmcp_client.rs:672-733`).
- **Demonstrated Token Impact:** For regular MCP servers, the same sentence (`This tool is part of plugin ...`) is appended to every child declaration. A 100-tool namespace with a short plugin name adds roughly 925-1,000 redundant heuristic tokens before other framing.
- **Remediation Seam:** For regular MCP servers whose tools share server-scoped membership, render provenance once in the namespace/server description and keep connector-specific Codex Apps attribution per tool.
- **Required Verification:** Expose 100 regular MCP tools under one plugin; assert the common provenance sentence appears once, while connector-specific app tools retain accurate per-tool membership.

### CF-094: Hidden MCP Tools Trigger Visible-Name Collision Renaming and Defeat Cache

- **Sources & Facet:** H034
- **Trigger, Mechanism & Root Cause:** `capture_binding_with_metadata` (`codex-rs/codex-mcp/src/connection_manager/tool_catalog.rs:283-315`) resolves sanitized-name collisions before visibility filtering. The trigger requires a hidden tool and visible tool whose sanitized names collide, followed by hidden-catalog churn that changes collision resolution for the visible declaration.
- **Demonstrated Token Impact:** The visible tool name changes despite no visible-catalog semantic change, invalidating request compatibility and prompt-cache/continuation reuse. Frequency depends on the specific hidden/visible collision and later hidden catalog update.
- **Remediation Seam:** Apply model visibility filtering first, and resolve name collisions only among tools that are actually visible to the model.
- **Required Verification:** Register a hidden tool with the same name as a visible tool; verify that the visible tool's name is not modified.

### CF-096: Interrupted Deferred Search Reconstructs as Successful Empty Output

- **Sources & Facet:** H041 (false-empty-recovery facet)
- **Trigger, Mechanism & Root Cause:** Rollout reconstruction converts an orphaned in-flight search into a synthetic empty completed result (`codex-rs/core/src/context_manager/normalize.rs:20-85`).
- **Demonstrated Token Impact:** After a crash between the persisted search call and its output, every prompt synthesizes `status: completed, tools: []`. If the task continues to need the deferred tool, the model must issue another search and then another follow-up inference.
- **Remediation Seam:** Reconstruct an orphaned client search as interrupted/failed (or resume it), not as a successful empty result, and preserve that status consistently in prompt history.
- **Required Verification:** Cold-resume a rollout ending after `ToolSearchCall`; assert the model sees an interrupted result and does not need a duplicate search to distinguish failure from zero matches.

### CF-097: Manual Local Compaction Invokes Model on Empty History

- **Sources & Facet:** H036 (empty history facet) (empty-history facet)
- **Trigger, Mechanism & Root Cause:** `CompactTask::run` (`codex-rs/core/src/tasks/compact.rs:28-72`) and `run_compact_task_inner_impl` send a full summarization request to the model even when session history contains 0 user/assistant messages.
- **Demonstrated Token Impact:** Wastes a full model request (prompt templates + instructions) performing a no-op summarization.
- **Remediation Seam:** Add an empty-history check in `CompactTask::run` to short-circuit and return success immediately without making a network request.
- **Required Verification:** Call compaction on a newly created empty thread; assert that no network request is made and compaction succeeds as a no-op.

### CF-100: Interrupted Fork Synthesis Can Append a Second Model-Visible Marker

- **Sources & Facet:** H074 (interrupted-fork boundary facet)
- **Trigger, Mechanism & Root Cause:** `append_interrupted_boundary` (`codex-rs/core/src/thread_manager.rs:2265-2334`) appends an abort marker without checking if a live interruption marker was already flushed.
- **Demonstrated Token Impact:** If a fork snapshots after the live interrupt path flushes its model-visible marker but before `TurnAborted`, `append_interrupted_boundary` unconditionally appends another marker. The fork therefore starts with two interruption messages for one event.
- **Remediation Seam:** Make interrupted-boundary synthesis prefix-idempotent by detecting an existing interruption marker before appending the synthetic marker and abort event.
- **Required Verification:** Fork during the marker-to-abort persistence interval; assert exactly one model-visible interruption marker and one terminal abort boundary.

### CF-101: Last-N Forks Drop Context Baselines While Retaining Context Fragments

- **Sources & Facet:** H075
- **Trigger, Mechanism & Root Cause:** When creating a Last-N fork (`codex-rs/core/src/agent/control/spawn.rs`), historical baseline markers are dropped, but rendered context fragments are retained. On turn 1 of the fork, the runtime cannot verify the baseline and re-injects current context fragments.
- **Demonstrated Token Impact:** Duplicates system context and configuration fragments in the child fork.
- **Remediation Seam:** Preserve baseline provenance markers when pruning items for Last-N forks, or canonicalize context fragments during fork initialization.
- **Required Verification:** Create a Last-N fork; verify that retained context fragments are recognized and not re-injected on turn 1.

### CF-104: Image Token Heuristics Diverge from Active GPT-5.6 Accounting

- **Sources & Facet:** H067
- **Trigger, Mechanism & Root Cause:** `codex-rs/core/src/context_manager/history.rs:709-717` uses a fixed 1,844 token estimate for images, whereas GPT-5.6 models use dynamic tile-based patch tokenization. This divergence causes the admission manager to either underestimate image cost (causing context window overflow) or overestimate it (causing premature compaction).
- **Demonstrated Token Impact:** Leads to context overflow rejections or premature compaction invocations on multimodal conversations.
- **Remediation Seam:** Adopt provider-aware image token calculation in `ContextManager` matching the active model's patch tiling formula.
- **Required Verification:** Submit images of various resolutions; verify that calculated token counts match official GPT-5.6 tile formulas.

### CF-105: Negative Catalog Output Limits Wrap to an Enormous Allowance (Complete)

- **Status:** **Complete** — current `HEAD` rejects negative catalog and remote-model truncation
  limits before model selection and can no longer construct a wrapped `TruncationPolicy`.
- **Current implementation:** `TruncationPolicyConfig::limit` is deserialized through a
  validating deserializer (`deserialize_nonnegative_limit`) that fails catalog load and remote
  `/models` decoding with `truncation policy limit must be nonnegative`. The
  `From<TruncationPolicyConfig> for TruncationPolicy` conversion in
  `codex-rs/protocol/src/protocol.rs` uses `usize::try_from` with a zero clamp so a
  programmatic negative value truncates loudly instead of disabling truncation entirely.
- **Preserved behavior:** Nonnegative limits, including zero and `i64::MAX`, round-trip
  unchanged. The bundled `models.json` catalog, custom `model_catalog_json` catalogs, the
  models cache, and the remote `/models` endpoint keep their existing wire shape; a rejected
  remote catalog still degrades through the existing fetch-failure fallback path.
- **Regression evidence:** `truncation_policy_rejects_negative_limits` and
  `truncation_policy_round_trips_nonnegative_limits` cover the serde boundary;
  `truncation_policy_conversion_never_wraps_negative_limits` and
  `truncation_policy_conversion_preserves_nonnegative_limits` cover the checked conversion;
  `model_catalog_json_rejects_negative_truncation_limit` proves a custom catalog fails config
  load before model selection; and
  `rejects_models_response_with_negative_truncation_limit` proves the remote `/models` decode
  fails before the catalog is applied.

### CF-109: Provider ID-Less Output Items Defeat Incremental Continuation

- **Sources & Facet:** H029
- **Trigger, Mechanism & Root Cause:** When an API provider omits item IDs in streaming events, `LastResponse` captures the ID-less item before the turn processor assigns local fallback IDs (`codex-rs/core/src/client.rs:1318-1348`). On the next turn, exact prefix comparison between history and `LastResponse` fails, forcing a full prompt resubmission instead of incremental continuation.
- **Demonstrated Token Impact:** Disables WebSocket incremental continuation and forces full context resubmission (often 50K+ tokens).
- **Remediation Seam:** Canonicalize item IDs in `map_response_events` before `LastResponse` is recorded, ensuring consistency with history.
- **Required Verification:** Stream responses from a provider omitting item IDs; verify that WebSocket continuation succeeds on the subsequent turn.

### CF-111: Parallel Hook Insertion Splices Server Order and Disables Continuation

- **Sources & Facet:** H071
- **Trigger, Mechanism & Root Cause:** The trigger requires parallel streamed tool-call siblings plus a matching hook completion inserted between those sibling call items (`codex-rs/core/src/stream_events_utils.rs:297-327`). That local insertion changes stored item order relative to the server `LastResponse`, so exact prefix comparison fails on the follow-up.
- **Demonstrated Token Impact:** For that interleaving, the next compatible WebSocket follow-up falls back to a full-context create rather than suffix continuation. Parallel calls without an intervening matching hook completion are excluded.
- **Remediation Seam:** Buffer completed hook context during tool execution and append them in a deterministic boundary after all active tools complete.
- **Required Verification:** Execute parallel tools alongside async hooks; verify that item ordering is preserved and incremental continuation remains active.

### CF-114: Usage Recomputation Counts Provider-Stripped Passthrough Metadata

- **Sources & Facet:** H090
- **Trigger, Mechanism & Root Cause:** `estimate_token_count` estimates tokens over raw rollout history containing passthrough metadata fields that are stripped before non-OpenAI request transmission (`codex-rs/core/src/client.rs:934-943`). This phantom token count can push a thread over threshold and trigger an unnecessary compaction.
- **Demonstrated Token Impact:** Triggers premature compaction based on metadata tokens that never actually reach the provider.
- **Remediation Seam:** Use a shared item normalizer for token recomputation that excludes provider-stripped metadata fields.
- **Required Verification:** Populate history with metadata-heavy items on a non-OpenAI provider; assert token estimation reflects only transmitted fields.

### CF-115: Model Switches Between Identical Base Instructions Append Full Duplicate Text

- **Sources & Facet:** H089 (equal instructions facet)
- **Trigger, Mechanism & Root Cause:** `ModelInstructionsState::render_diff` (`codex-rs/core/src/context/world_state/model.rs:44-59`) compares model slugs rather than instruction contents. Switching between bundled models (e.g. Sol to Luna) that share identical base instruction text appends the full 17.7 KiB instruction block again.
- **Demonstrated Token Impact:** Appends ~4,500 tokens of duplicate base instructions on every intra-family model switch.
- **Remediation Seam:** Compare the content hash of base instructions in `ModelInstructionsState` rather than comparing model slugs.
- **Required Verification:** Switch between two models sharing identical base instructions; verify that no instruction update fragment is emitted.

### CF-116: Remote V2 Discards Complete Compaction Response on Missing Terminal Event

- **Sources & Facet:** H113 (lost-terminal retry facet)
- **Trigger, Mechanism & Root Cause:** If Remote V2 receives all compaction chunks and a completed `Compaction` item but the stream terminates without an explicit `response.completed` event (`codex-rs/core/src/compact_remote_v2.rs:75-81`), it treats the attempt as failed, discards the compaction, and automatically resubmits the entire compaction request.
- **Demonstrated Token Impact:** The client-observed duplicate generation is proven: after receiving and discarding one complete compaction item, it submits the full semantic compaction again. The exact billing/commit status of the ambiguous first provider attempt is not established by the client-side test.
- **Remediation Seam:** Persist a pending compaction operation identity before dispatch and reconcile through a provider-supported idempotency or completed-response contract. Without that contract, quarantine the ambiguous operation instead of automatically regenerating it; do not blindly accept an unterminated item as committed.
- **Required Verification:** Exercise a complete `Compaction` item followed by missing `response.completed`; assert there is no automatic second generation without reconciliation. Separately verify provider-reported usage/billing when that contract becomes available.

### CF-117: Realtime Handoff Duplicates User Text in Input and Transcript Delta

- **Sources & Facet:** #27
- **Trigger, Mechanism & Root Cause:** In `realtime_conversation.rs::realtime_delegation_from_handoff` (`1647-1671`), the handoff collector includes `input_transcript` inside `active_transcript`. Handoff rendering then places the exact same user text into both the `<input>` and `<transcript_delta>` XML blocks of the handoff message.
- **Demonstrated Token Impact:** Directly duplicates user speech text in every voice-to-text model handoff payload.
- **Remediation Seam:** Exclude the final user input transcript from `active_transcript` delta rendering when constructing the handoff prompt.
- **Required Verification:** Perform a voice handoff to text turn; assert that the user message appears only in `<input>` and not duplicated in `<transcript_delta>`.

### CF-120: Assistant-Only Realtime Transcript Tail Starts a Coding-Model Turn

- **Sources & Facet:** H045 (assistant-only transcript-tail facet)
- **Trigger, Mechanism & Root Cause:** `flush_realtime_transcript_tail` routes remaining audio transcripts through a full model inference pass even if the tail contains only assistant speech (`codex-rs/core/src/realtime_conversation.rs:1997-2011`).
- **Demonstrated Token Impact:** With experimental realtime enabled and `flushTranscriptTailOnSessionEnd = true`, any nonempty assistant-only tail is wrapped as synthetic user input and starts an ordinary coding-agent request even though it contains no new user-authored request.
- **Remediation Seam:** Make transcript-tail admission role-aware: persist/fan out assistant-only tail state without starting `RegularTask`; invoke the model only when the tail contains new user speech.
- **Required Verification:** Close a realtime session with only assistant entries after the last handoff; assert transcript state is retained but no `/responses` request is issued. Keep a user-tail control that still samples once.

### CF-123: Shared Remote Compaction Trimmer Stops at the First Non-Output Group (Complete)

- **Status:** **Complete** — the shared Remote V1/V2 trimmer continues scanning past newer
  non-output groups and reaches older rewritable outputs across user/assistant boundaries.
- **Current implementation:** `trim_function_call_history_for_context_window` in
  `codex-rs/core/src/compact_remote.rs` walks `history_item_groups` in reverse, tracking
  position with a `traversed_items` cursor so a non-rewritable group is skipped while older
  rewritable output groups remain reachable until the reduction target is met.
- **Preserved behavior:** Non-rewritable groups themselves are left intact, and harness
  metadata on rewritten outputs is preserved.
- **Regression evidence:** `shared_trimmer_reaches_outputs_behind_newer_message_groups` in
  `codex-rs/core/src/compact_remote_metadata_tests.rs` places an oversized removable output
  behind a newer user message and asserts the traversal rewrites the older output;
  `rewritten_output_preserves_harness_metadata` covers metadata preservation.

### CF-124: Remote V2 Retains Prior Local Compaction Summaries as Real User Messages

- **Sources & Facet:** H017
- **Trigger, Mechanism & Root Cause:** Local compaction summaries carry `compaction.summary` metadata but lack special user-message markers (`codex-rs/core/src/context/compaction_summary.rs:17-35`). Remote V2 (`compact_remote_v2.rs:480-564`) misclassifies them as real user messages and retains them alongside the new compaction summary.
- **Demonstrated Token Impact:** Accumulates multiple redundant historical summaries in post-compaction context.
- **Remediation Seam:** Filter out prior `CompactionSummary` user messages when selecting retained messages in `build_v2_compacted_history`.
- **Required Verification:** Run Remote V2 compaction on a session that previously underwent local compaction; verify old local summary is replaced.

### CF-125: Manual Remote V2 Compaction Samples Pristine History

- **Sources & Facet:** H018
- **Trigger, Mechanism & Root Cause:** The manual Remote V2 path selected by `CompactTask::run` (`codex-rs/core/src/tasks/compact.rs:28-62`) enters `run_remote_compact_v2_attempt` (`codex-rs/core/src/compact_remote_v2_attempt.rs:41-108`) without a pristine-history preflight and sends base instructions plus a compaction trigger. Automatic compaction is excluded because it does not trigger on pristine history.
- **Demonstrated Token Impact:** Wastes a full remote model call when compacting a thread that has no compactable history.
- **Remediation Seam:** Add a preflight check in `run_remote_compact_v2_attempt` to return early with an empty compaction record if history contains no compressible items.
- **Required Verification:** Invoke manual Remote V2 compaction on a pristine thread and assert no network request; retain an automatic-compaction control showing that path is not implicated.
