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

**Summary:** 58 active demonstrated bug facets. Facet labels are authoritative where the same
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

**Evidence convention:** “Generated,” “resent,” or “extra request” describes client-observed model work. Provider billing is claimed only where usage or a provider contract establishes it; otherwise the record explicitly marks billing as unknown or conditional.

**Impact tiers:** **High** means a full request, compaction, large payload, or multiplicative recurrence; **Medium** is bounded or feature/failure-gated but still material; **Low** is small per occurrence or a narrow interleaving. Tiers estimate token reduction, not correctness severity.

| Canonical ID | Title & Summary | Reachability / Trigger | Expected Token Impact | Primary Fix Seam |
| --- | --- | --- | --- | --- |
| CF-009 | Filtered forks inherit stale parent token usage and compact prematurely | Filtered legacy fork near compaction threshold | High | `codex-rs/core/src/agent/control/spawn.rs::keep_forked_rollout_item` |
| CF-010 | Rollout reconstruction re-injects initial context baseline | Reconstruction from full context without user boundary | High | `codex-rs/core/src/session/rollout_reconstruction.rs::finalize_active_segment` |
| CF-014 (single-server repeated identity) | Explicit single-server MCP listings repeat server identity per descriptor | Explicit single-server MCP list | Low | `codex-rs/core/src/tools/handlers/mcp_resource.rs` |
| CF-015 (projection) | Generic MCP resource reads flatten binary content into JSON/base64 text | Generic MCP binary/media resource read | High | `codex-rs/core/src/tools/handlers/mcp_resource/read_mcp_resource.rs` |
| CF-016 (typed-resource) | Normal MCP results JSON-stringify embedded resources and media | MCP result embeds resource/media | High | `codex-rs/protocol/src/models.rs::convert_mcp_content_to_items` |
| CF-017 (premature-compaction) | Request-fit overestimation triggers avoidable compaction | Near-threshold request shrinks during normalization | High | `codex-rs/core/src/session/turn.rs::run_turn` & `build_prompt` |
| CF-018 | Local compaction resubmits full prompt iteratively and retains failed output | Local compaction rejection or post-output stream failure | High | `codex-rs/core/src/compact.rs::run_compact_task_inner_impl` |
| CF-019 (final-payload-cap) | Final Tool Output Can Exceed Its Advertised Per-Output Cap | High-cardinality/token-dense structured output | High | `truncate_function_output_payload` |
| CF-020 (stale-notice/re-expansion) | Tool output lifecycle re-expands on resume and retains stale notices | Resume under changed output policy or image retention | High | `codex-rs/core/src/session/mod.rs::prepare_conversation_items_for_history` |
| CF-021 (audio-accounting) | Remote V2 undercharges audio while normal history overcharges durationless audio | Remote V2 audio or valid durationless audio | High | `codex-rs/utils/audio/src/lib.rs::estimate_audio_token_count` |
| CF-022 | Temporary structured turn timeout fails to cancel active model inference | Temporary title/recap timeout or stale result | Medium | `codex-rs/tui/src/temporary_structured_request.rs::run_temporary_structured_turn` |
| CF-025 | Queued turn dispatches before durable deletion commits | Queue dispatch + delete failure/crash window | High | `codex-rs/state/src/runtime/queued_items.rs` & `codex-rs/ext/queue/src/service.rs` |
| CF-027 | Cold resume resets shared rollout budget ledger to zero | Rollout budget enabled + cold resume | High | `codex-rs/core/src/agent/control.rs::AgentControl` & `core/src/rollout_budget.rs` |
| CF-028 | Same-window resume re-arms one-shot reminder delivery state | Same-window resume after reminder delivery | Low | `codex-rs/core/src/state/session.rs::SessionState` & `apply_rollout_reconstruction` |
| CF-030 | Direct interruption paths can immediately relaunch an active goal | Direct goal interrupt without prior TUI pause | High | `codex-rs/ext/goal/src/extension.rs::GoalExtension::on_thread_idle` |
| CF-033 | Goal tool responses echo full objective and derived state | Goal create/update with nontrivial objective | Low | `codex-rs/ext/goal/src/tool.rs::GoalToolExecutor` |
| CF-034 (swallowed-accounting-error) | Swallowed goal accounting persistence error launches duplicate turn | Goal accounting persistence failure | High | `codex-rs/ext/goal/src/runtime.rs::account_active_goal_progress` |
| CF-038 (expired-lease duplicate-sampling) | Phase 1 Memory Leases Can Expire Before Queued Jobs Start | Memories; batch >=17; jobs exceed one-hour lease | Medium | `memories/write/src/phase1.rs` |
| CF-044 | Exhausted memory consolidation retries continue claiming workers | Exhausted Phase 2 job becomes claimable again | High | `codex-rs/state/src/runtime/memories.rs::try_claim_global_phase2_job` |
| CF-045 (checkpoint-retired discovery-schema) | Memory Phase 1 Reuploads Discovery Schemas Retired by Compaction | Memories + compacted rollout with search schemas | High | `ToolSearchCall` |
| CF-046 | MCP resource handlers bypass memory external-context suppression | MCP resource output remains memory-eligible | Medium | `codex-rs/core/src/tools/handlers/mcp_resource.rs::run_resource_operation` |
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
| CF-077 (post-render completion-cap) | Rendered Subagent Completion Items Can Exceed Their 1K Cap | Escaping-heavy completion or long agent path | Medium | Render the complete V1/V2 completion envelope first, then enforce the 1K model-visible cap across escaped payload, markers, and agent-path metadata |
| CF-078 | Nonterminal subagent errors are published as terminal failures | Nonterminal agent `ErrorEvent` | Medium | `codex-rs/core/src/agent/status.rs::agent_status_from_event` |
| CF-082 | Inline review findings are persisted twice in parent history | Inline review completion | Medium | `codex-rs/core/src/tasks/review.rs::exit_review_mode` |
| CF-083 | Reusable Guardian delta turns re-emit root evidence and advance on parse failure | Reusable Guardian follow-up or parse retry | High | `codex-rs/core/src/guardian/prompt.rs` & `review_session.rs` |
| CF-086 (literal schema-restatement) | Synchronous Guardian Supplies the Output Contract Twice | Synchronous full Guardian review | Low | `guardian_output_schema()` |
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
| CF-105 (negative-catalog-limit) | Negative Catalog Output Limits Wrap to an Enormous Allowance | Negative catalog truncation limit | High | `TruncationPolicy` |
| CF-109 | Provider ID-less output items defeat incremental continuation | Provider returns missing/empty output item ID | High | `codex-rs/core/src/client.rs::map_response_events` |
| CF-111 | Hook completion between parallel streamed calls disables continuation | Parallel calls + hook completes between siblings | High | `codex-rs/core/src/stream_events_utils.rs` & `core/src/hook_runtime.rs` |
| CF-114 | Usage recomputation counts provider-stripped passthrough metadata | Non-OpenAI normalization + metadata-heavy history | High | `codex-rs/core/src/client.rs` & `core/src/session/mod.rs` |
| CF-115 | Model switches between identical base instructions append full duplicate text | Switch between models with identical instructions | Medium | `codex-rs/core/src/context/world_state/model.rs::ModelInstructionsState::render_diff` |
| CF-116 (lost-terminal regeneration) | Missing terminal delivery discards a complete compaction item and regenerates it | Remote V2 receives compaction item but loses terminal | High | `codex-rs/core/src/compact_remote_v2.rs` |
| CF-117 | Realtime handoff duplicates user text in input and transcript delta | Realtime voice-to-text handoff | Low | `codex-rs/core/src/realtime_conversation.rs` |
| CF-120 (assistant-only transcript-tail) | Assistant-Only Realtime Transcript Tail Starts a Coding-Model Turn | Experimental realtime tail flush; assistant-only tail | Medium | `RegularTask` |
| CF-123 | Shared Remote V1/V2 trimmer stops at first non-output group | Remote V1/V2 over-window trim meets newer non-output | High | `codex-rs/core/src/compact_remote.rs::trim_function_call_history_to_fit_context_window` |
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

### CF-009: Filtered Legacy Forks Inherit Stale Parent Token Usage and Compact Prematurely

- **Sources & Facet:** H048, H073
- **Trigger, Mechanism & Root Cause:** When spawning a legacy subagent fork (`codex-rs/core/src/agent/control/spawn.rs:63-100, 877-980`), `keep_forked_rollout_item` filters model-visible history items, but retains the parent's `TokenCount` event. The child session adopts this inflated parent token count upon initialization (`codex-rs/core/src/session/mod.rs:1388-1398`), causing the admission logic (`turn.rs:155-176`) to trigger an immediate, unnecessary compaction before the child's first turn.
- **Demonstrated Token Impact:** Triggers an avoidable full compaction request (10K+ tokens) on subagent startup even when child history easily fits within the context window.
- **Remediation Seam:** Strip inherited `TokenCount` events when filtering rollout items for a fork, and recalculate initial token usage directly from the retained child items.
- **Required Verification:** Create a subagent fork from a large parent session where history is filtered down to a small prompt; verify the child does not trigger compaction on turn 1.

### CF-010: Rollout Reconstruction Re-Injects Initial Context Baseline

- **Sources & Facet:** #11, H076, H074 (context baseline facet)
- **Trigger, Mechanism & Root Cause:** During rollout reconstruction (`codex-rs/core/src/session/rollout_reconstruction.rs:107-135`), `finalize_active_segment` promotes reference baselines for `WorldState` and `TurnContext` only across user turn boundaries. If a session is reconstructed from a prefix without a user turn boundary (e.g. following token-budget compaction or fork initialization), the baseline is not recognized, causing the session to append a duplicate initial context bundle on the next turn (`codex-rs/core/src/session/mod.rs:3993-4041`).
- **Demonstrated Token Impact:** Re-injects the complete system instructions, permissions, and environment bundle (several thousand tokens) into model context.
- **Remediation Seam:** Update `finalize_active_segment` so that surviving full `WorldState` and `TurnContext` snapshots establish the baseline even in the absence of an explicit user boundary.
- **Required Verification:** Reconstruct a session from a compacted context prefix without a user message; verify that turn 1 does not re-emit system and environment fragments.

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

### CF-014: MCP Resource Listings Repeat Server Identity per Descriptor

- **Sources & Facet:** H059 (resource listing facet), H060, H062 (repeated-identity facet)
- **Trigger, Mechanism & Root Cause:** In explicit single-server resource/template listings (`codex-rs/core/src/tools/handlers/mcp_resource.rs:104-186,346-362`), the call already identifies the server and the result repeats it at top level, yet every descriptor repeats the same server identity again. Flattened all-server listings are excluded because they have no single top-level server and need per-descriptor ownership.
- **Demonstrated Token Impact:** Bloats tool output with redundant JSON keys and server strings, wasting model-visible context tokens.
- **Remediation Seam:** For explicit single-server listing responses, omit per-descriptor server identity and retain one top-level server field. Keep ownership on each descriptor for flattened all-server results.
- **Required Verification:** Call resource listing for a server returning 50 resources; assert that server name and base URI appear once in the group envelope, not repeated 50 times.

### CF-015: Generic MCP Resource Reads Flatten Binary Content into JSON/Base64 Text

- **Sources & Facet:** #24 (generic resource read facet), H059 (oversized read facet) (projection facet)
- **Trigger, Mechanism & Root Cause:** `ReadMcpResourceHandler` (`codex-rs/core/src/tools/handlers/mcp_resource/read_mcp_resource.rs:63-94`) and `mcp_resource.rs::serialize_function_output` serialize the complete server metadata, URI, and binary/blob contents into unstructured JSON text. Image and binary resources are emitted as base64 strings in text blocks rather than typed content items.
- **Demonstrated Token Impact:** Massive token inflation from base64 strings and JSON wrapper boilerplate in prompt history.
- **Remediation Seam:** Project generic MCP resource reads into typed `FunctionCallOutputContentItem` objects (typed images, blobs, text) rather than JSON stringified envelopes.
- **Required Verification:** Read an image via `read_mcp_resource`; verify that the tool output produces a typed image content item rather than JSON text containing a base64 string.

### CF-016: Normal MCP Results JSON-Stringify Embedded Resources and Media

- **Sources & Facet:** #24 (CallToolResult facet), H063 (typed-resource facet)
- **Trigger, Mechanism & Root Cause:** `codex-rs/protocol/src/models.rs::convert_mcp_content_to_items` (`2243-2384`) treats embedded MCP resource blocks and binary content as unknown types and JSON-stringifies them into raw text content items, destroying their typed modality structure before model prompt construction.
- **Demonstrated Token Impact:** Forces the model to parse stringified JSON envelopes containing base64 data, significantly inflating token counts and degrading multimodal comprehension.
- **Remediation Seam:** In `convert_mcp_content_to_items`, map embedded MCP images and resource content directly to typed `FunctionCallOutputContentItem::Image` and structured resource items.
- **Required Verification:** Return an MCP `CallToolResult` containing an embedded image; assert that the downstream message item is a typed image rather than a JSON text block.

### CF-017: Request-Fit Overestimation Triggers Avoidable Compaction

- **Sources & Facet:** #3, #15, H066, H107 (premature-compaction facet)
- **Trigger, Mechanism & Root Cause:** `ContextManager::estimate_token_count_with_base_instructions` calculates context fit before final request-scoped media normalization and filtering occur (`codex-rs/core/src/session/turn.rs:155-188, 350-499`). When history contains media or items that will be stripped or scaled down during request construction (`codex-rs/core/src/client.rs:904-984`), admission overestimates the request size and triggers an unnecessary compaction invocation.
- **Demonstrated Token Impact:** Launches an expensive full compaction inference (several thousand tokens) when the actual prepared request would have fit comfortably within the model window.
- **Remediation Seam:** Run admission checks against the exact normalized `ResponsesRequest` payload that will be transmitted, after request-level media scaling and filtering have been applied.
- **Required Verification:** Construct a turn where raw history exceeds the threshold but normalized request context is below threshold; verify that no compaction is triggered.

### CF-018: Local Compaction Resubmits Full Prompt Iteratively and Retains Failed Output

- **Sources & Facet:** #6, H035, H074 (failed local compaction facet)
- **Trigger, Mechanism & Root Cause:** `run_compact_task_inner_impl` (`codex-rs/core/src/compact.rs:245-390, 735-806`) implements an iterative retry loop that removes one oldest message group per rejected submission and resubmits nearly the entire prompt again. If a stream fails midway, partial assistant text is flushed to history before terminal completion, polluting history with failed compaction attempts.
- **Demonstrated Token Impact:** Multiple near-full-window model resubmissions upon rejection, and failed compaction output persists in history requiring subsequent cleanup.
- **Remediation Seam:** Implement a `LocalCompactionPlan` that removes whole message groups in bulk to reach a safe target budget in one step. Stage compaction output in memory and commit to `Session` history only upon complete, successful terminal response.
- **Required Verification:** Simulate context rejection during local compaction; verify that history is reduced in a single step rather than one item at a time, and failed partial streams do not write to rollout history.

### CF-019: Final Tool Output Can Exceed Its Advertised Per-Output Cap

- **Sources & Facet:** #2 (nominal 10K/effective 12K facet), H058, H077, H078 (final-payload-cap facet)
- **Trigger, Mechanism & Root Cause:** Output truncation uses approximate byte heuristics (`bytes / 4`), applies `* 1.2`, and does not account for JSON escaping or array framing (`codex-rs/utils/output-truncation/src/lib.rs:94-224`). Token-dense text or high-cardinality arrays can exceed the nominal 10K token policy.
- **Demonstrated Token Impact:** A nominal 10K-token output can serialize to about 99K heuristic tokens with many one-byte items; unsupported-audio projection can reach about 276K heuristic tokens in one output. The oversized item is then persisted and sent in the required follow-up request.
- **Remediation Seam:** Make `truncate_function_output_payload` the final authority after modality projection: remove the `1.2` expansion, charge complete item/wrapper framing and item count, assign nonzero structural cost, and remeasure the provider-visible payload.
- **Required Verification:** Cover token-dense text, 12K one-byte items, empty encrypted items, zero-duration audio, and unsupported-audio projection; assert the final serialized function output stays within the configured 10K cap.

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

### CF-025: Queued Turn Dispatches Before Durable Deletion Commits

- **Sources & Facet:** H003
- **Trigger, Mechanism & Root Cause:** `QueuedItemService::start` (`codex-rs/ext/queue/src/service.rs:391-448`) starts the model turn via `start_if_idle` before deleting the queued row from SQLite (`codex-rs/state/src/runtime/queued_items.rs:152-160`). If the process crashes or DB deletion fails, the item remains in the queue as pending and is dispatched again on the next idle state.
- **Demonstrated Token Impact:** Causes duplicate model turns and full task replays upon crash or database write error.
- **Remediation Seam:** Implement a state machine in `QueuedItemsRuntime` (`Pending -> Claimed { turn_id } -> Completed`) so items are marked Claimed before turn dispatch and reconciled on startup.
- **Required Verification:** Inject a database deletion failure after queue turn dispatch; verify that the item is in Claimed state and is not re-executed on subsequent idle events.

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

### CF-030: Goal Continuation Ignores Interrupted Idle Cause and Resumes Active Turn

- **Sources & Facet:** #26
- **Trigger, Mechanism & Root Cause:** `GoalExtension::on_thread_idle` (`codex-rs/ext/goal/src/extension.rs:148-158`) automatically calls `continue_if_idle` without distinguishing `ThreadIdleCause::Interrupted`. The direct bug applies to app-server or other interruption paths that do not first execute the TUI-specific goal-pause operation; TUI flows that pause the goal before interrupt are excluded.
- **Demonstrated Token Impact:** Defeats user cancellation and immediately launches an unwanted model inference turn.
- **Remediation Seam:** In `on_thread_idle`, match on `ThreadIdleCause` and only trigger automatic goal continuation when the cause is `ThreadIdleCause::Completed`.
- **Required Verification:** Interrupt an active turn in a goal session; verify that the goal pauses and does not launch a new continuation turn.

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

### CF-044: Exhausted Memory Consolidation Retries Continue Claiming Workers

- **Sources & Facet:** #14
- **Trigger, Mechanism & Root Cause:** `try_claim_global_phase2_job` (`codex-rs/state/src/runtime/memories.rs:1076-1219, 1305-1325`) queries for consolidation jobs whose backoff timer has expired, but fails to check `retry_remaining > 0`. A job that has failed repeatedly continues to be claimed and launched indefinitely.
- **Demonstrated Token Impact:** Failed memory consolidation jobs spawn endless model worker tasks, repeatedly burning tokens on unresolvable failures.
- **Remediation Seam:** In `try_claim_global_phase2_job`, add `AND retry_remaining > 0` to the SQL query and mark jobs as permanently failed when retries reach zero.
- **Required Verification:** Fail a Phase 2 job until its retry count is 0; assert that subsequent consolidation runs do not claim or spawn workers for that job.

### CF-045: Memory Phase 1 Reuploads Discovery Schemas Retired by Compaction

- **Sources & Facet:** H042 (checkpoint-retired discovery-schema facet)
- **Trigger, Mechanism & Root Cause:** Phase 1 extraction processes raw rollout events (`codex-rs/memories/write/src/phase1.rs:289-313`), including tool search schemas that were later compacted away in active history.
- **Demonstrated Token Impact:** Each historical tool-search result can contribute up to 32 KiB of schemas to a fresh Phase 1 model request even after the active conversation checkpoint removed it. Multiple searches accumulate without an aggregate historical-schema filter.
- **Remediation Seam:** Build the Phase 1 projection from reconstructed active history, or at minimum omit pre-checkpoint `ToolSearchCall` and `ToolSearchOutput` records while preserving semantically necessary tool evidence.
- **Required Verification:** Compact a rollout containing multiple discovery results, then run Phase 1; assert checkpoint-retired schemas are absent while surviving active evidence remains.

### CF-046: MCP Resource Handlers Bypass Memory External-Context Suppression

- **Sources & Facet:** H061
- **Trigger, Mechanism & Root Cause:** `codex-rs/core/src/tools/handlers/mcp_resource.rs:280-324` returns standard tool outputs without marking them as external context. Consequently, they bypass the external-context memory pollution guard in `codex-rs/core/src/tools/registry.rs:780-786`, making the session eligible for memory extraction that ordinary MCP outputs would suppress.
- **Demonstrated Token Impact:** Triggers unnecessary background memory extraction passes on threads containing third-party resource dumps.
- **Remediation Seam:** Mark MCP resource tool outputs with `is_external_context = true` so the memory guard correctly identifies external context.
- **Required Verification:** Read an MCP resource; verify that the turn is flagged as containing external context and memory extraction is suppressed.

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

### CF-077: Rendered Subagent Completion Items Can Exceed Their 1K Cap

- **Sources & Facet:** H024 (post-render completion-cap facet)
- **Trigger, Mechanism & Root Cause:** Truncation applies to raw completion text before JSON/XML escaping and agent path metadata are added (`codex-rs/core/src/session_prefix.rs:9-24`).
- **Demonstrated Token Impact:** A raw completion truncated to 900 heuristic tokens can render to about 1,832 tokens with quotes or newlines and about 5,432 tokens with NUL escaping; unbounded duplicated agent paths add further model-visible bytes.
- **Remediation Seam:** Render the complete V1/V2 completion envelope first, then enforce the 1K model-visible cap across escaped payload, markers, and agent-path metadata.
- **Required Verification:** Use quote-heavy, newline-heavy, NUL-heavy payloads and long agent paths; assert the final rendered fragment—not only the raw payload—stays within the item cap.

### CF-078: Nonterminal Subagent Errors are Published as Terminal Failures

- **Sources & Facet:** H025
- **Trigger, Mechanism & Root Cause:** `agent_status_from_event` (`codex-rs/core/src/agent/status.rs:6-21`) treats all `ErrorEvent` occurrences as terminal subagent failures, even when the error is non-fatal and the subagent continues running, leading to conflicting results and unnecessary retries.
- **Demonstrated Token Impact:** Triggers false-failure handling and redundant agent launches while the original agent is still running.
- **Remediation Seam:** Check `ErrorEvent::is_terminal` before transitioning agent status to `Failed`.
- **Required Verification:** Emit a recoverable error event in a subagent; verify that agent status remains running until terminal completion.

### CF-082: Inline Review Findings are Persisted Twice in Parent History

- **Sources & Facet:** #21
- **Trigger, Mechanism & Root Cause:** `exit_review_mode` (`codex-rs/core/src/tasks/review.rs:217-264`) records review findings first in a synthetic user review-result envelope and then immediately duplicates the text in a plain assistant message.
- **Demonstrated Token Impact:** Every completed inline review doubles the token footprint of its findings in parent context.
- **Remediation Seam:** Store review findings once using a single canonical review message envelope.
- **Required Verification:** Complete an inline review; verify that findings text appears exactly once in the parent session history.

### CF-083: Reusable Guardian Delta Turns Re-Emit Root Evidence and Advance on Parse Failure

- **Sources & Facet:** H012
- **Trigger, Mechanism & Root Cause:** In reusable Guardian sessions (`codex-rs/core/src/guardian/prompt.rs:142-178`), the delta cursor only tracks transcript items, while root authorization and environment context are re-appended outside the delta branch on every turn. Furthermore, if parsing fails, the cursor advances anyway, causing missed deltas.
- **Demonstrated Token Impact:** Repeats thousands of tokens of root evidence across every reviewed tool turn.
- **Remediation Seam:** Maintain an atomic cursor that tracks both transcript and root evidence, ensuring root context is not re-appended on delta turns.
- **Required Verification:** Execute multiple Guardian review passes in a reusable session; verify root evidence is sent once and subsequent turns contain only new tool deltas.

### CF-086: Synchronous Guardian Supplies the Output Contract Twice

- **Sources & Facet:** H092 (literal schema-restatement facet)
- **Trigger, Mechanism & Root Cause:** Guardian prompt includes both structured JSON schema and detailed prose instructions restating the output schema (`codex-rs/core/src/guardian/prompt.rs:775-843`).
- **Demonstrated Token Impact:** Every synchronous full Guardian review repeats the four field names, types, and complete enum sets in both base-instruction prose and `text.format`; the overlapping prose is about 54 heuristic input tokens per uncached attempt and repeats on retries.
- **Remediation Seam:** Keep read-only investigation guidance and the low-risk shortcut, but remove the literal property/type/enum restatement already carried by `guardian_output_schema()`.
- **Required Verification:** Capture a synchronous Guardian request and assert each schema field and enum set is defined only in `text.format`, while behavioral guidance remains in instructions; keep Guardian V2 excluded.

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

### CF-105: Negative Catalog Output Limits Wrap to an Enormous Allowance

- **Sources & Facet:** H079 (negative catalog-limit facet)
- **Trigger, Mechanism & Root Cause:** Truncation limits deserialize as signed `i64` and cast unchecked to `usize` (`codex-rs/protocol/src/openai_models.rs:363-380`), allowing negative values to disable truncation.
- **Demonstrated Token Impact:** A catalog `limit: -1` reaches `config.limit as usize`, becoming `usize::MAX` on supported 64-bit targets. Direct MCP/function output then bypasses the intended per-output truncation and retains all feasible text up to unrelated transport limits.
- **Remediation Seam:** Validate catalog and remote-model truncation limits as nonnegative before conversion; use a checked conversion to `TruncationPolicy` and reject invalid catalogs.
- **Required Verification:** Load a custom and remote model catalog with negative byte/token limits; assert validation fails before model selection and no wrapped policy is constructed.

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

### CF-123: Shared Remote Compaction Trimmer Stops at the First Non-Output Group

- **Sources & Facet:** #18
- **Trigger, Mechanism & Root Cause:** Both Remote V1 and Remote V2 call `trim_function_call_history_to_fit_context_window` (`codex-rs/core/src/compact_remote.rs:399-454`; `codex-rs/core/src/compact_remote_v2_attempt.rs:41-65`). Once over-window rewriting starts, reverse traversal breaks at the newest group that is not a rewritable output, leaving older outputs that the existing policy already considers removable.
- **Demonstrated Token Impact:** Leaves large historical tool outputs in the compaction prompt, risking window overflow and failed compaction.
- **Remediation Seam:** Continue scanning all historical tool output groups across user/assistant message boundaries until target reduction is achieved.
- **Required Verification:** For both Remote V1 and Remote V2, place an oversized removable output behind a newer non-output group; assert traversal reaches and rewrites the older output.

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
