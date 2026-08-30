# Token-Consumption Audit Evidence

This document preserves common-fix analysis, all 127 canonical records, archived
dispositions, source mappings, original findings, and fleet adjudication. It is
evidence rather than an active backlog. Use [BUGS.md](BUGS.md) for demonstrated
defects, [HARDENING.md](HARDENING.md) for defensive work, and
[DESIGN_DECISIONS.md](DESIGN_DECISIONS.md) for unresolved behavior or provider
contracts.

### Merged findings

#### CF-001 — Temporary metadata workers use a bounded auxiliary profile

- **Classification:** H057's discarded reasoning output is an actual token bug. The
  amount of project, AGENTS, and World State bootstrap a title/recap worker should
  retain is a design decision and requires output-quality validation.
- **Sources:** #5 (worker-profile/input-bootstrap facet), H057.
- **Common fix:** Introduce a typed metadata-worker profile at
  `codex-rs/tui/src/temporary_structured_request.rs::start_temporary_thread` /
  `start_structured_turn`, carry it through
  `codex-rs/app-server/src/request_processors/thread_processor.rs::thread_start_inner`,
  and apply it in `codex-rs/core/src/session/session.rs::Session::new`. The proven fix
  is to disable reasoning summaries that the consumer discards. Any omission of
  project/AGENTS/World State bootstrap or change to inherited effort belongs to an
  explicitly approved metadata-worker policy while preserving model/provider and
  permission isolation.
- **Why merged:** #5's oversized bootstrap input and H057's discarded reasoning output
  are both consequences of creating a tiny title/recap job with ordinary root-session
  policy. One proof-bearing workload profile removes both before request construction.
- **Scope/impact:** The repository's large root `AGENTS.md` is an example, not a
  universal floor. Luna titles already force low effort; recaps and fallback-title
  models have broader inherited-effort exposure. Stale cancellation is separate
  (CF-022).
- **Required coverage:** Capture title and recap requests and assert bounded worker
  instructions, no tools, and no reasoning summary; keep an ordinary-thread control
  that still inherits configured reasoning. Assert omission of project/World State
  context or an explicit effort only after the metadata-worker policy is approved, and
  retain a quality control for every context class that policy removes.

#### CF-002 — Goal continuations separate static context from bounded deltas

- **Sources:** H051; H110 (repeated continuation-reference/reread-incentive facet).
- **Common fix:** Split
  `codex-rs/ext/goal/src/steering.rs::continuation_prompt` into a versioned static
  `GoalContextRevision` and a small `GoalContinuationDelta`, and make
  `codex-rs/ext/goal/src/runtime.rs::GoalRuntimeHandle::continue_if_idle` inject the
  static revision only once per goal revision/context window.
- **Why merged:** The repeated 6.3K policy/objective and the repeated imperative to read
  the materialized objective are the same bytes in the same persisted continuation
  item. One versioned projection removes both repetitions without changing initial
  attachment materialization.
- **Scope/impact:** Growth is logical model context, not guaranteed HTTP/WebSocket
  retransmission. The model is encouraged, not runtime-forced, to reread. The first
  fetch and attachment bounds remain CF-036.
- **Required coverage:** Run at least ten continuations and assert one static
  policy/objective/reference per revision/window plus bounded deltas; cover XML
  escaping, pause/clear, resume, compaction reinjection, and an unchanged file
  reference.

#### CF-003 — Phase 1 uses a sanitized citation- and modality-aware extraction projection

- **Sources:** H103 (Phase 1 extraction facet), H105.
- **Common fix:** Make
  `codex-rs/memories/write/src/phase1.rs::sanitize_response_item_for_memories` and
  `serialize_filtered_rollout_response_items` the single Phase 1 projection: strip
  memory-citation markup, replace image/audio payloads with bounded modality
  placeholders, and apply the same rule inside function/custom-tool outputs before
  `build_stage_one_input_message`.
- **Why merged:** Citation markup and retained media have one route into extraction:
  the filtered `ResponseItem` vector is JSON-serialized into one outer text item.
  Sanitizing that boundary removes both from every Phase 1 request, including
  pre-compaction rollout lines.
- **Scope/impact:** Memory generation and an eligible rollout are required. A 1 MiB
  image is truncated by the current Phase 1 allowance rather than transmitted in full,
  but base64 can still occupy most of the retained input and displace useful text.
- **Required coverage:** Inspect the actual outbound Phase 1 input for assistant
  citations plus user/tool image and audio content; assert no citation tags, data URLs,
  or base64 survive, useful text/call relationships remain, and a preceding
  `Compacted` record cannot resurrect raw media.

#### CF-004 — All hook-generated model context consumes one aggregate budget

- **Sources:** #8, H007.
- **Common fix:** Add one hard `HookModelContextBudget` spanning
  `codex-rs/hooks/src/output_spill.rs::HookOutputSpiller::maybe_spill_additional_contexts`,
  `maybe_spill_prompt_fragments`,
  `codex-rs/core/src/hook_runtime.rs::record_additional_contexts`,
  `drain_async_hook_results`, and Stop prompt admission in
  `codex-rs/core/src/session/turn.rs::run_turn`.
- **Why merged:** Ordinary additional context and the fragments flattened into one Stop
  prompt are independently granted a full per-handler allowance today. Charging every
  sync/async fragment to one turn ledger removes both handler-count multipliers; zero
  can no longer disable the hard cap.
- **Scope/impact:** Requires enabled trusted handlers returning nonempty context or
  blocking reasons. This bounds one lifecycle's bytes; it does not bound the number of
  Stop-driven model rounds (CF-050).
- **Required coverage:** Replace per-fragment limit tests with aggregate sync, async,
  mixed, zero-config, and Stop-batch cases; charge XML/spill metadata and prove later
  async completions see only the remaining allowance.

#### CF-005 — Additional context is canonicalized before framing, fingerprinting, and budgeting

- **Sources:** #28 (additional-context facet), H083.
- **Common fix:** Introduce a prepared batch in
  `codex-rs/core/src/state/additional_context.rs::AdditionalContextStore::prepare`,
  using one validator/renderer shared with
  `codex-rs/context-fragments/src/additional_context.rs::additional_context_body` /
  `AdditionalContextUserFragment::matches_text` and
  `codex-rs/app-server/src/request_processors/turn_processor.rs::map_additional_context`.
  Validate or encode keys, enforce key/count/aggregate limits, render/truncate first,
  then fingerprint the final model-visible projection.
- **Why merged:** Raw-tail changes outside the rendered 1K value, delimiter-bearing
  keys that evade recognition, and unbounded key/batch bytes all arise before there is
  one canonical admitted representation. One preparation step fixes every member.
- **Scope/impact:** H083's deterministic duplicate needs an untrusted malformed key and
  local compaction; application entries share the admission problem but not that exact
  path. The app-server field is experimental, but Core must enforce the invariant for
  direct callers too.
- **Required coverage:** Assert render-equivalent updates do not append, validate long
  UTF-8 and delimiter keys, enforce entry/aggregate caps, and prove local compaction
  retains exactly one correctly classified copy.

#### CF-006 — Compaction replacement is authoritative for current additional context

- **Sources:** H082, H084.
- **Common fix:** In
  `codex-rs/core/src/session/additional_context.rs::Session::rehydrate_additional_context_for_compaction`,
  `codex-rs/core/src/session/mod.rs::Session::replace_compacted_history`, and
  `apply_rollout_reconstruction`, remove every prior `additional_content.*` projection,
  insert exactly one canonical current item per key, persist the same full baseline
  even for `DoNotInject`, and insert nothing for cleared keys.
- **Why merged:** H082 loses the baseline while H084 retains obsolete versions because
  replacement is additive rather than authoritative. Replacing the whole projection
  and persisting its baseline resolves both in one compaction-boundary operation.
- **Scope/impact:** H082 is limited to non-token-budget compaction followed by immediate
  resume/fork; H084 additionally requires client-developer retention, which is
  default-disabled. Malformed-key admission is CF-005.
- **Required coverage:** Exercise local, remote-v1, and remote-v2 compact/resume/fork;
  include successive values, explicit clear, retention enabled, and reconstruction,
  asserting one current copy per key and zero for cleared keys.

#### CF-007 — Code Mode callbacks retain immutable cell and turn ownership

- **Sources:** #12 (stale cross-turn delivery facet), H028.
- **Common fix:** Bind each cell to an immutable owner turn/generation/router in
  `codex-rs/core/src/tools/code_mode/delegate.rs::CodeModeDispatchBroker::start_turn_worker`,
  `CodeModeSessionDelegate::invoke_tool` / `notify`, and
  `CoreTurnHost::notify`; make
  `codex-rs/core/src/tools/code_mode/mod.rs::interrupt_active_cells` target only cells
  owned by the interrupted turn.
- **Why merged:** Delayed notifications and delayed nested tool calls are both consumed
  by whichever request-scoped worker wins the shared broker. One owner-bound dispatch
  contract prevents later-turn routing and unrelated interruption for both message
  kinds.
- **Scope/impact:** Requires a delayed callback and an overlapping/later request. Feature
  enablement is session-invariant; a later effective Direct-mode request is the
  relevant no-worker case. Notification volume remains CF-072.
- **Required coverage:** Let cell A yield while turn B installs a different router;
  prove A's call/notification cannot use B, interrupt B leaves A intact, no-worker
  behavior is bounded, and concurrent cells preserve ownership.

#### CF-008 — Auxiliary consumers do not request reasoning summaries they discard

- **Sources:** #31, H014, H037.
- **Common fix:** In
  `codex-rs/core/src/client.rs::ModelClient::build_responses_request`, suppress
  `reasoning.summary` when request metadata identifies
  `CodexResponsesRequestKind::Compaction` or `ThreadSource::GuardianReview`; retain
  reasoning effort and ordinary-turn behavior.
- **Why merged:** H014 and H037 are exact duplicates across local/remote compaction, and
  #31 creates the same unwanted request field for synchronous Guardian. Every path
  reaches the shared request builder with enough metadata to apply one policy.
- **Scope/impact:** No waste occurs when the model lacks summary support or the effective
  setting is `none`. Any generated plaintext summary is unused; provider emission and
  billing per attempt are conditional.
- **Required coverage:** Request-body matrix: ordinary turns keep `detailed`, while
  synchronous Guardian and local/remote-v1/remote-v2 compaction omit summary but retain
  effort; feed summary events/items and verify accepted output and usage behavior are
  unchanged.

#### CF-009 — Filtered legacy forks recompute token usage from child history

- **Sources:** H048, H073.
- **Common fix:** At
  `codex-rs/core/src/agent/control/spawn.rs::keep_forked_rollout_item` and fork
  construction, remove inherited `TokenCount` events after response-history filtering
  and recompute/estimate usage from the filtered child history before
  `codex-rs/core/src/session/mod.rs::Session::apply_rollout_reconstruction` can drive
  admission.
- **Why merged:** The two hypotheses are exact duplicates: same `InitialHistory::Forked`
  vector, same retained parent token snapshot, and same pre-first-sample compaction.
- **Scope/impact:** FullHistory is exposed when filtering removes model-visible material;
  Last-N only when a token event survives the cut. Paginated forks and ordinary resume
  are excluded. The established impact is one unnecessary first-turn compaction.
- **Required coverage:** Near-threshold materially filtered FullHistory and Last-N forks
  must not compact when the child prompt fits; ordinary resume keeps exact usage and
  paginated behavior remains unchanged.

#### CF-010 — Surviving full context snapshots establish reconstruction baselines

- **Sources:** #11, H076, H074 (complete context-only baseline facet).
- **Common fix:** Update
  `codex-rs/core/src/session/rollout_reconstruction.rs::finalize_active_segment` so a
  surviving full `WorldState` after the newest compaction, together with `TurnContext`,
  promotes a reference baseline even without a user boundary; do not count the segment
  as a user turn for rollback.
- **Why merged:** FullHistory filtering, pristine TokenBudget compaction, and a complete
  pre-user context prefix all retain the same proof that canonical initial context was
  installed. Recognizing that full snapshot prevents the same reinjection in every
  case.
- **Scope/impact:** Patches alone, snapshots superseded by a newer checkpoint, and
  prefixes actually missing required records are excluded. #11 affects qualifying
  FullHistory forks; H076 is default-off TokenBudget before the first user turn.
- **Required coverage:** Port full-snapshot/removed-boundary cases; add pristine
  TokenBudget resume and completed FullHistory fork cases plus a complete context-only
  prefix, asserting exactly one environment/permissions/developer bundle.

#### CF-011 — Tool exposure uses one budget-aware catalog plan

- **Sources:** #1, H038.
- **Common fix:** Add an authoritative planner adjacent to
  `codex-rs/core/src/tools/spec_plan.rs::build_model_visible_specs` that computes final
  serialized cost, prioritizes explicit/required tools, and assigns direct/deferred/
  hidden exposure under a hard aggregate budget; feed the same retained set to
  `register_code_mode_executors` and
  `codex-rs/code-mode-protocol/src/description.rs::build_exec_tool_description`.
- **Why merged:** #1 is unbounded direct exposure and H038 is unconditional deferral of
  catalogs that would fit. Both are opposite outcomes of the same binary exposure
  policy; only a budget-aware planner fixes both without trading one for the other.
- **Scope/impact:** Native-search-deferred MCP definitions are not all present in every
  request. The 66 MiB catalog is a stress witness, not a normal estimate; direct versus
  deferred benefit depends on latency, cache, and usage.
- **Required coverage:** Small and explicitly selected catalogs stay direct when they
  fit; large mixed catalogs have a deterministic ceiling and discoverable remainder;
  search-unavailable excess fails/omits clearly; CodeModeOnly uses the identical plan.

#### CF-012 — Deferred discovery has lifecycle-aware history and compaction projection

- **Sources:** #7, H039.
- **Common fix:** Give active discovery bounded keyed state adjacent to
  `codex-rs/core/src/context_manager/history.rs::ContextManager`: key exact results by
  query/catalog revision/schema digest, replace repeats, mark consumed versus still
  needed, omit consumed schema bodies from local/v1/v2 compaction input, and reinstall
  the latest unconsumed result after replacement.
- **Why merged:** Duplicate ordinary-request copies and compaction uploads are the same
  stored `ToolSearchOutput` objects. One authoritative lifecycle/projection removes
  both while preserving a result still needed by the immediate continuation.
- **Scope/impact:** Each result is already bounded to 32 leaves/32 KiB; repeated or
  multiple results create the aggregate cost. Blind stripping is unsafe for an
  unconsumed mid-turn result.
- **Required coverage:** Exact repeated searches retain one full body; distinct results
  survive; local/v1/v2 compaction omits consumed bodies but preserves a latest live
  result; retry/fallback does not multiply the sanitized projection.

#### CF-013 — External tool schemas preserve executable constraints

- **Sources:** H050, H068.
- **Common fix:** Preserve supported JSON-Schema constraints in
  `codex-rs/tools/src/json_schema.rs::parse_tool_input_schema` /
  `sanitize_json_schema` (or a canonical raw external-schema wrapper), and perform
  provider-capability lowering only at
  `codex-rs/tools/src/responses_api.rs::tool_definition_to_responses_api_tool`, with the
  same schema available for optional local validation before MCP/dynamic dispatch.
- **Why merged:** MCP and dynamic tools use the same parser and serializer. H050's
  removed numeric bounds are one instance of H068's shared loss of numeric, string,
  array, and boolean-schema semantics.
- **Scope/impact:** Corrective inference requires the executor to enforce a lost
  constraint and the model to violate it. Ordinary `{"type":"boolean"}` already works;
  boolean-schema `true`/`false` is the narrower subcase.
- **Required coverage:** Round-trip numeric, string-length, array-cardinality, and
  boolean-schema forms for both MCP and dynamic tools; test server/client enforcement
  and make any provider fallback explicit.

#### CF-014 — MCP resource listing is budget-aware, grouped, and resumable

- **Classification:** Repeating server identity per descriptor is an actual token bug.
  Cursor validation, aggregate limits, and resumable fan-out are hardening.
- **Sources:** H059 (resource-listing facet), H060, H062.
- **Common fix:** Replace split list paths with one pager in
  `codex-rs/core/src/tools/handlers/mcp_resource.rs` and
  `codex-rs/codex-mcp/src/binding_clients.rs`: validate cursor size/repetition/page
  state, stop before descriptor/serialized-byte budgets, return a Codex-owned composite
  continuation, and group descriptors by server so identity is not repeated per item.
- **Why merged:** Destructive all-server truncation, unguarded single-server cursors,
  and repeated server fields are all decided by the resource-list pager and response
  envelope before generic serialization.
- **Scope/impact:** Single-server calls currently preserve a cursor but bypass shared
  guards; all-server calls have internal guards but discard continuation. Repeated
  identity mainly displaces useful descriptors within the existing output cap.
- **Required coverage:** Use the same guards for resources/templates and single/all
  server paths; reject repeated cursors, resume fan-out without prefix replay, keep
  ownership unambiguous, and replace middle-truncation assertions with page-envelope
  assertions.

#### CF-015 — Generic MCP resource reads use typed progressive output

- **Classification:** JSON/base64 projection and repeated wrappers are actual token
  bugs. Progressive continuation is hardening because an extra model call depends on a
  later retry; the exact range/cursor API is an implementation contract.
- **Sources:** #24 (generic `read_mcp_resource` facet), H059 (oversized-read facet).
- **Common fix:** Replace
  `codex-rs/core/src/tools/handlers/mcp_resource/read_mcp_resource.rs::ReadMcpResourceHandler::handle_call`
  through `mcp_resource.rs::serialize_function_output` with a resource-specific typed
  envelope that budgets before serialization and spills/pages oversized content behind
  a bounded range or continuation handle.
- **Why merged:** JSON/base64 textification and futile repeat of the same head/tail are
  produced by the same `ReadResourcePayload` conversion. One progressive typed
  projector removes wrapper waste and makes omitted content recoverable.
- **Scope/impact:** Server-defined chunk URIs and specialized readers mitigate some
  resources; the generic tool has no progression. Ordinary MCP `CallToolResult`
  resource blocks are CF-016.
- **Required coverage:** Text, image/audio, and blob reads produce bounded typed
  envelopes; oversized reads return non-overlapping continuation chunks; server/URI
  metadata is not repeated in every chunk.

#### CF-016 — Normal MCP results preserve resource structure and annotation hints

- **Classification:** JSON/base64 fallback for typed embedded resources is an actual
  token bug. How audience and priority hints influence filtering or budget order is a
  design decision; they are hints, not authority.
- **Sources:** #24 (ordinary `CallToolResult` projection facet), H063.
- **Common fix:** Replace
  `codex-rs/protocol/src/models.rs::convert_mcp_content_to_items` with a typed
  projection for embedded resources/resource links. Preserve audience and priority
  metadata through lowering, but adopt model-audience filtering or priority-aware
  budgeting only under an explicit projection policy.
- **Why merged:** Unknown-resource fallback to JSON text and deletion of standard
  annotations occur at the same normal-model projection boundary. Preserving structure
  there lets one downstream pass avoid base64 text and low-priority displacement.
- **Scope/impact:** Annotations are client hints, not confidentiality controls.
  `structuredContent` and Code Mode follow different paths; the concrete exposure is
  unstructured direct Responses output.
- **Required coverage:** Embedded image/audio becomes typed media or compact
  descriptors, links retain essential identity, annotations survive lowering, and
  structuredContent/Code Mode behavior remains unchanged. Add filtering, reordering,
  or "high priority wins" assertions only after an annotation policy is approved.

#### CF-017 — Admission uses the exact prepared request that will be sent

- **Classification:** H066/H107 overestimation that automatically launches compaction
  is an actual token bug. #3/#15 underestimation is hardening because extra inference
  depends on provider rejection and caller resubmission.
- **Sources:** #3, #15, H066, H107.
- **Common fix:** Build one reusable prepared request in
  `codex-rs/core/src/session/turn.rs::run_turn` / `build_prompt` and
  `codex-rs/core/src/client.rs::build_responses_request` after target-model
  normalization and after incoming context, user input, hooks, activations, tools, and
  output schema are known; use the same object for admission and transmission,
  including remote-v1/v2 compaction attempts.
- **Why merged:** #3/#15 underestimate late-added fields, while H066/H107 overestimate
  media that the current request removes. All four are divergence between the object
  measured and the object sent; one final prepared representation fixes both
  directions.
- **Scope/impact:** Underestimation can cause rejection, futile compaction, and caller
  resubmission; overestimation causes one unnecessary compaction. If immutable request
  fields alone cannot fit, fail directly rather than compacting history.
- **Required coverage:** Cross the window with tools/schema and with new turn input;
  prove compaction precedes the first normal sample and input appears once. Conversely,
  normalize unsupported media below threshold and prove no compaction. Cover ordinary,
  remote-v1, and remote-v2 construction.

#### CF-018 — Local compaction has one authoritative reduced plan and atomic commit

- **Sources:** #6, H035, H074 (failed-local-compaction-output facet).
- **Common fix:** Refactor
  `codex-rs/core/src/compact.rs::run_compact_task_inner_impl` around a
  `LocalCompactionPlan`: snapshot once, reduce whole item groups in bulk, stage
  `drain_to_completed` output until terminal success, build replacement from the exact
  reduced snapshot, budget the full replacement, and commit once through
  `Session::replace_compacted_history`.
- **Why merged:** The per-item retry loop, resurrection of trimmed messages, and
  persistence of failed-attempt output all result from lacking one attempt object that
  owns source, reduction, generated output, and replacement.
- **Scope/impact:** #6 requires multiple removable groups after provider rejection;
  H035 resurrects only eligible user items from the untrimmed source; H074's facet
  needs output followed by stream failure before `Completed`.
- **Required coverage:** Force several removals and bounded attempts; assert removed
  items stay absent, media-only/tiny envelopes fit the final replacement, and
  `OutputItemDone` followed by error leaves no live, persisted, or reconstructed text.

#### CF-019 — Per-output limits bound the final model-visible payload

- **Sources:** #2 (nominal-10K/effective-12K facet), H058, H077, H078.
- **Common fix:** Make
  `codex-rs/core/src/context_manager/history.rs::truncate_function_output_payload`,
  backed by `codex-rs/utils/output-truncation/src/lib.rs`, the authoritative finalizer:
  charge the complete serialized wrapper and every item, use a model/provider token
  strategy with a conservative fallback, impose structural/count cost for zero-cost
  items, remove the arbitrary `* 1.2`, and rerun after request-scoped modality
  projection.
- **Why merged:** The 12K allowance, dense-text undercount, uncharged structured-item
  framing, zero-cost media/encrypted blocks, and unsupported-media expansion are all
  ways the pre-projection approximation diverges from one final function-output value.
- **Scope/impact:** Applies to one function/custom-tool output. Common text may remain
  below 10K actual tokens; dense encodings and high-cardinality arrays are the strong
  witnesses. Aggregate multiplication across separate outputs remains CF-103.
- **Required coverage:** Token-dense text, one-byte arrays, empty encrypted blocks,
  zero-duration audio, markers, and unsupported-audio projection must all leave the
  actual outbound output within the nominal policy; include a 10K integration request.

#### CF-020 — Tool outputs are canonicalized once at the durable history boundary

- **Classification:** #25 resume re-expansion and H080 stale notices are actual token
  bugs. H064's later-model media behavior is a design decision because additional
  inference depends on a later capability switch and reread policy.
- **Sources:** #25, H064, H080.
- **Common fix:** At
  `codex-rs/core/src/session/mod.rs::prepare_conversation_items_for_history` /
  `record_prepared_conversation_items`, apply the existing retention policy once,
  derive resize notices only from the retained payload, and append that identical
  retained representation to live history and rollout so resume cannot re-expand it.
  Do not change whether unsupported MCP media is permanently removed or retained for a
  later capable model until a durable-media policy is approved. Under such a policy,
  preserve typed durable media and keep request-specific unsupported-media substitution
  in `ContextManager::for_prompt_annotated` on a clone.
- **Why merged:** Re-expansion on resume, destructive MCP media loss, and stale resize
  notices are all ordering errors around the same durable boundary. Persisting one
  retained canonical payload while making modality projection reversible resolves all
  three.
- **Scope/impact:** #25 affects uncompacted suffixes and policy changes; H064 requires
  direct MCP media with no independent copy; H080 is default-off and needs more images
  than later retention preserves.
- **Required coverage:** Resume a small-policy output under a larger policy without
  re-expansion, and verify notices name only retained images in live, rollout, and
  resumed history. Test text-only-to-capable-model media recovery without another MCP
  call only after the durable-media policy is approved.

#### CF-021 — Remote V2 retention uses the shared full-item estimator with robust audio cost

- **Classification:** #16/H106 audio misaccounting is an actual token bug. H016's full
  envelope accounting is hardening for extreme retained histories; roles and wrappers
  are required content, not redundant content.
- **Sources:** #16, H016, H106.
- **Common fix:** Make
  `codex-rs/utils/audio/src/lib.rs::estimate_audio_token_count` robust to valid
  durationless containers (bounded packet/frame fallback), use it from
  `codex-rs/core/src/context_manager/history.rs::estimate_item_token_count`, and make
  `codex-rs/core/src/compact_remote_v2.rs::truncate_retained_messages` budget every
  retained item with that same full serialized-item estimator.
- **Why merged:** H106 is the bad audio primitive; #16 bypasses it by charging retained
  audio as zero; H016 independently omits roles/wrappers/IDs/metadata. Routing Remote
  V2 through one corrected full-item cost path resolves all three without parallel
  accounting rules.
- **Scope/impact:** Remote V2 retention is the #16/H016 scope. H106 also affects normal
  history for an accepted container lacking declared duration. Tiny-message and media
  examples are tokenizer/container dependent; a one-token floor is not a serialized
  64K guarantee.
- **Required coverage:** Add a deterministic durationless valid audio fixture/probe;
  assert normal retention uses duration-derived cost, Remote V2 charges and drops audio
  when needed, many tiny/metadata-heavy messages remain inside the full-item budget,
  and the post-compaction request does not immediately overflow.

### Additional canonical records

The active classifications in `BUGS.md`, [HARDENING.md](HARDENING.md), and
[DESIGN_DECISIONS.md](DESIGN_DECISIONS.md) are authoritative. The titles and seams
below preserve the prior implementation analysis; they are not a second backlog, and
the three archived records remain only for traceability.

| Canonical ID | Source/facet | Title | Primary fix seam |
| --- | --- | --- | --- |
| CF-022 | #5 — stale cancellation | Cancel stale temporary structured turns | `codex-rs/tui/src/temporary_structured_request.rs::run_temporary_structured_turn` owning exact thread/turn interrupt and bounded shutdown |
| CF-023 | H001 | Give automation recurrences stable cache affinity | `app-server-protocol/src/protocol/v2/thread.rs::ThreadStartParams` -> `core/src/session/session.rs::Session::new` / `CodexClient::with_prompt_cache_key_override` |
| CF-024 | H002 | Enforce turn idempotency keys before routing | `app-server`/Core turn admission in `core/src/session/turn_input.rs::start_or_steer` plus unique queue enqueue |
| CF-025 | H003 | Make queued-turn dispatch a durable state machine | `codex-rs/state/src/runtime/queued_items.rs` and `codex-rs/ext/queue/src/service.rs` (`Pending -> Claimed -> Completed`) |
| CF-026 | H004 | **Product decision required:** Define automation memory eligibility | Approve the automation/source memory-mode matrix, then persist it in thread creation metadata and enforce it in `state/src/runtime/memories.rs::claim_stage1_jobs_for_startup` |
| CF-027 | H020 | Persist and restore shared rollout-budget authority | `core/src/agent/control.rs::AgentControl` / `core/src/rollout_budget.rs` and cold-resume reconstruction |
| CF-028 | H021 | Restore same-window reminder delivery state | `core/src/state/session.rs::SessionState`, `apply_rollout_reconstruction`, time/token reminder state |
| CF-029 | #30 | **Product decision required:** Project reminders as bounded latest state | Approve the append-only audit and cache-compatibility behavior, then implement request-time typed state projection in `core/src/session/time_reminder.rs` and `session/rollout_budget.rs` |
| CF-030 | #26 | Gate goal continuation by the idle cause | `codex-rs/ext/goal/src/extension.rs::GoalExtension::on_thread_idle` exhaustive `ThreadIdleCause` handling |
| CF-031 | H052 | Propagate goal-budget authority to descendants | `ext/goal` execution scope carried through `core/src/agent/control/spawn.rs` |
| CF-032 | H053 | Synchronize goal mutations with the active turn | Transactional `ext/goal/src/api.rs` / `runtime.rs` mutation plus exact-turn cancellation |
| CF-033 | H054 — create/update projection | Use operation-specific goal tool responses | `codex-rs/ext/goal/src/tool.rs::GoalToolExecutor` model-facing DTOs |
| CF-034 | H055 | Stop implicit continuation after goal-accounting failure, then apply an approved recovery policy | `ext/goal/src/runtime.rs::account_active_goal_progress` and lifecycle stop decisions |
| CF-035 | H056 | Bound standalone web-search history input | `codex-rs/ext/web-search/src/history.rs::recent_input` with aggregate `SearchHistoryBudget` |
| CF-036 | H110 — initial materialization | Preserve typed goal attachments through materialization | `codex-rs/tui/src/goal_files.rs::materialize_goal_draft` typed inline/referenced result and aggregate attachment limits |
| CF-037 | H098 | Run memory startup once per root thread | `app-server/src/request_processors/turn_processor.rs::turn_start_inner` / `memories/write/src/start.rs` |
| CF-038 | H101 | Acquire and heartbeat Phase 1 leases just in time | `memories/write/src/phase1.rs::claim_startup_jobs` / `run_jobs` / `job::sample` |
| CF-039 | H099 — Phase 1 prompt | Condense the Phase 1 extraction rubric | `memories/write/templates/memories/stage_one_system.md` with `phase1.rs::output_schema` |
| CF-040 | H102 | **Product decision required:** Define memory quota failure policy | Approve behavior for Codex-backend quota lookup errors versus custom-provider `NotApplicable`, then implement typed admission in `codex-rs/memories/write/src/guard.rs::rate_limits_check` / `start_memories_startup_task` |
| CF-041 | H097 — DB-backed corpus | Bound the DB-backed Phase 2 corpus | `state/src/runtime/memories.rs::get_phase2_input_selection` and both workspace writers |
| CF-042 | H099 — Phase 2 prompt | Render a mode-specific bounded Phase 2 prompt | `memories/write/src/prompts.rs::build_consolidation_prompt` |
| CF-043 | H100 | Enforce a hard Phase 2 execution deadline | `memories/write/src/phase2.rs::agent::loop_agent` total lifecycle budget |
| CF-044 | #14 | Honor exhausted Phase 2 retry budgets | `state/src/runtime/memories.rs::try_claim_global_phase2_job` requiring `retry_remaining > 0` |
| CF-045 | H042 | Apply compaction checkpoints to memory extraction input | `memories/write/src/phase1.rs::serialize_filtered_rollout_response_items` using reconstructed active history |
| CF-046 | H061 | Mark MCP resource output as external context | `core/src/tools/handlers/mcp_resource.rs::run_resource_operation` output provenance before registry admission |
| CF-047 | H097 — external-agent import | Bound external-agent memory imports | `external-agent-migration/src/memory_import.rs::import` / `replace_project_resources` |
| CF-048 | H099 — memory read-path prompt | Archived: existing memory read-path bounds cover the claim | No token fix; retain the existing 64 KiB rejection and 2,500-token summary truncation |
| CF-049 | H103 — ordinary model history | Strip parsed memory citations from normal model projection | `core/src/stream_events_utils.rs::handle_output_item_done` plus sanitized prompt/compaction projection |
| CF-050 | #9 | Bound Stop continuation rounds | `core/src/session/turn.rs::run_turn` with host-enforced `StopContinuationState` |
| CF-051 | H005 | Give internal workers an explicit hook profile | `core/src/session/mod.rs::build_hooks_config` keyed by Review/Memory session source |
| CF-052 | H006 | Invalidate asynchronous hooks removed by rollback | `hooks/src/engine/command_runner.rs`, `core/src/session/handlers.rs::thread_rollback`, and hook-result drain |
| CF-053 | H112 | Expire completed Stop prompts at V2 checkpoints | Hook-run lifecycle metadata plus `core/src/compact_remote_v2.rs::build_v2_compacted_history` |
| CF-054 | #28 — environment delta | Render environment common fields as deltas | `core/src/context/world_state/environment.rs::EnvironmentsState::render_diff` |
| CF-055 | H085 | Reject or encode top-level-null World State snapshots | `ext/extension-api/src/contributors/world_state.rs::WorldStateSectionContribution::new` and Core section adapter |
| CF-056 | H086 | Suppress retained World State guidance on re-enable | `core/src/context/world_state/mod.rs::WorldState::render_history_diff` and history-aware step recording |
| CF-057 | #13 | Bound total exposure of one `skills.read` resource | `ext/skills/src/tools/read.rs::ReadTool::handle` / `page_response` and provider resource cap |
| CF-058 | H093 — same-turn aggregate | Aggregate explicit skill and plugin activation admission | `core/src/session/turn.rs::build_skills_and_plugins` after all activation producers assemble |
| CF-059 | H093 — cross-turn repetition | Avoid persisting full unchanged activation bodies | Skills/plugin activation fingerprint state restored across resume/compaction |
| CF-060 | H094 | Render skill-catalog updates as entry deltas | `ext/skills/src/world_state.rs` / `world_state_catalogs.rs` structured catalog snapshots |
| CF-061 | H095 — endpoint recommendations | Make endpoint plugin recommendations lazy and bounded | `core/src/context/recommended_plugins_instructions.rs` / initial-context contribution |
| CF-062 | H095 — legacy discovery | Query and paginate legacy plugin discovery | `core/src/tools/handlers/list_available_plugins_to_install.rs` and its tool spec |
| CF-063 | H096 | Remove shareable repeated guidance and account the complete rendered skill catalog | `ext/skills/src/render.rs::render_combined_available_skills` final serialized accounting |
| CF-064 | #4 — Unified Exec wait | Keep Unified Exec waits host-owned until completion | `core/src/unified_exec/process_manager.rs::exec_command` / `write_stdin` completion subscription |
| CF-065 | #4 — Code Mode wait | Keep Code Mode waits host-owned until completion | `core/src/tools/code_mode/wait_handler.rs::CodeModeWaitHandler::handle_call` / `CodeModeService::wait` |
| CF-066 | H008 | Normalize terminal output before model projection | `core/src/tools/context.rs::ExecCommandToolOutput::to_response_item` |
| CF-067 | H009 | Reconcile Unified Exec handles with durable history | `core/src/unified_exec/process_manager.rs`, shutdown, and rollout reconstruction handle validity |
| CF-068 | H010 | Finalize Unified Exec output only after producer closure | `process_manager.rs::collect_output_until_deadline` / `refresh_process_state` |
| CF-069 | H011 | Advertise non-TTY session capabilities accurately | `core/src/tools/handlers/shell_spec.rs`, `default_tty`, and `process_manager.rs::write_stdin` |
| CF-070 | H026 | Avoid hybrid Code Mode input-schema duplication | `core/src/tools/spec_plan.rs::build_model_visible_specs` / `tools/src/code_mode.rs::augment_tool_spec_for_code_mode` |
| CF-071 | H027 | Reconcile Code Mode state at checkpoints and resume | Local/remote compaction, rollout reconstruction, and `CodeModeService` generation policy |
| CF-072 | #12 — notification budget | Bound Code Mode notification size, count, and backlog | `code-mode-runtime/src/runtime/callbacks.rs::notify_callback` and dispatch broker queue |
| CF-073 | H072 | Propagate fatal asynchronous tool errors | `core/src/session/turn.rs::drain_in_flight` / `core/src/tools/parallel.rs` |
| CF-074 | #23 | Use sequenced agent completion result references | `core/src/agent/status.rs`, V1 `wait_agent`, and V2 `list_agents` |
| CF-075 | H022 | Admit queued V2 completion on the next first request | Session mailbox admission in `core/src/session/turn_input.rs` / `turn.rs` |
| CF-076 | H023 | Rearm V1 completion watchers for reused agents | `core/src/agent/control.rs` watcher lifecycle and `send_input` |
| CF-077 | H024 | Budget the final rendered completion envelope | `core/src/session_prefix.rs`, `SubagentNotification`, and `InterAgentCompletionMessage` |
| CF-078 | H025 | Keep nonterminal errors out of final agent status | `core/src/agent/status.rs::agent_status_from_event` |
| CF-079 | H047 | Harden nested-parent completion delivery against residency eviction | `core/src/agent/control/residency.rs` descendant-aware unloadability or durable parent mail |
| CF-080 | #4 — agent wait | Make agent waits completion-driven rather than model-polled | `core/src/tools/handlers/multi_agents_common.rs` / V1 wait status subscription |
| CF-081 | #22 | Give detached review a bounded review-specific context | `app-server/src/request_processors/turn_processor.rs::start_detached_review` / `core/src/thread_manager.rs` |
| CF-082 | #21 | Store inline review output once | `core/src/tasks/review.rs::exit_review_mode` |
| CF-083 | H012 | Track the complete reusable Guardian prompt state | `core/src/guardian/prompt.rs` and `review_session.rs` atomic prompt cursor |
| CF-084 | H070 | Review parallel actions from one pre-wave history snapshot | `GuardianReviewContext` creation before streamed sibling calls are recorded |
| CF-085 | H091 | Coalesce adjacent Guardian text items | Shared renderer used by `core/src/guardian/prompt.rs` and Guardian V2 classifier construction |
| CF-086 | H092 | Remove the duplicate Guardian output contract | `core/src/guardian/prompt.rs::guardian_output_contract_prompt` versus `guardian_output_schema` |
| CF-087 | H109 | Hard-cap fully rendered Guardian instructions | Sync Guardian policy renderer and `guardian-v2/src/async_scorer/config.rs::render_classifier_instructions` |
| CF-088 | H013 | Exclude the current V2 action from its transcript | `guardian-v2/src/async_scorer/transcript.rs` keyed by reviewed `call_id` |
| CF-089 | H015 | Stop Guardian generation after the first classification | `guardian-v2/src/async_scorer/sampler.rs` provider output cap or immediate cancellation |
| CF-090 | H108 | Deduplicate Guardian image sources | `guardian-v2/src/async_scorer/transcript.rs::TranscriptConfig::images` canonical image fingerprint |
| CF-091 | H031 | Gate provisional MCP catalogs on freshness and callability | `codex-mcp/src/connection_manager/tool_catalog.rs::capture_binding_with_metadata` |
| CF-092 | H032 | Publish live MCP catalog and metadata revisions | `codex-mcp` refresh publisher for `tools/list_changed` and session-expiry recovery |
| CF-093 | H033 | Render plugin provenance once per MCP namespace | `codex-mcp/src/rmcp_client.rs::add_plugin_provenance_to_tool` / namespace coalescing |
| CF-094 | H034 | Normalize visible MCP tool names after visibility filtering | `codex-mcp/src/connection_manager/tool_catalog.rs::capture_binding_with_metadata` |
| CF-095 | H040 | Report bounded deferred-search partiality | `tools/src/tool_discovery.rs::bound_tool_search_output` structured completeness result |
| CF-096 | H041 | Recover orphaned client searches without false success | `core/src/session/rollout_reconstruction.rs` before generic call-output normalization |
| CF-097 | H036 — empty-history facet | Skip empty local compaction | `core/src/tasks/compact.rs::CompactTask::run` / empty-history preflight before network activity; unchanged non-empty history remains a design question |
| CF-098 | #29 | Exclude retained user suffixes from compaction summary input | `core/src/compact.rs::run_compact_task_inner_impl` paired summary-source/retention policy |
| CF-099 | H074 — compound context/checkpoint persistence; H113 — failed checkpoint persistence | Persist compound compaction and context state atomically | `Session::replace_compacted_history` / `persist_rollout_items` and `thread-store/src/local/live_writer.rs` committed compound records with propagated persistence failure |
| CF-100 | H074 — interrupted-fork boundary | Make interrupted-fork boundary synthesis idempotent | `core/src/thread_manager.rs::append_interrupted_boundary` |
| CF-101 | H075 | Canonicalize context retained by Last-N forks | `core/src/agent/control/spawn.rs` fork sanitization with contextual-fragment provenance |
| CF-102 | H081 | Archived from token backlog: track reconstruction ordering as non-token correctness | `core/src/session/rollout_reconstruction.rs::ActiveReplaySegment` |
| CF-103 | #2 — aggregate retained outputs | Enforce an aggregate retained tool-output budget | `core/src/context_manager/history.rs::ContextManager::record_items_with_metadata` plus rebuild paths |
| CF-104 | H067 | Use provider-aware image token estimates | Model/provider image-cost metadata consumed by history, truncation, compaction, and Guardian |
| CF-105 | H079 — negative catalog limit | Reject negative signed truncation policies | Catalog ingestion and checked conversion to `TruncationPolicy` |
| CF-106 | H065 | Keep MCP telemetry out of model context | `core/src/tools/context.rs::McpToolOutput::response_payload` |
| CF-107 | #19 | Use an append-only Responses Lite catalog layout | `core/src/client.rs::ModelClient::build_responses_request` |
| CF-108 | #20 | Support explicit prompt-cache breakpoints | `codex-api/src/common.rs` request types and breakpoint-capable input content |
| CF-109 | H029 | Canonicalize missing output item IDs once | `core/src/client.rs::map_response_events` before `LastResponse` and history diverge |
| CF-110 | H030 | Validate service-tier continuation compatibility | `core/src/client.rs::responses_request_properties_match` after provider-contract confirmation |
| CF-111 | H071 | Buffer hook context until ordered tool drain | `core/src/stream_events_utils.rs`, tool runtime, and hook-result recording boundary |
| CF-112 | H114 — schema-only pre-compaction | Preserve V2 continuation across schema-only compaction | `responses_request_properties_match` with a provider-contracted compaction `text.format` rule |
| CF-113 | H088 | Archived pending evidence: trace Copilot WebSocket normalization impact | `codex-api/src/endpoint/responses_websocket.rs::ResponsesWebsocketConnection::stream_request` |
| CF-114 | H090 | Recompute usage from provider-normalized model input | Shared model-visible item normalizer used by request construction and history estimation |
| CF-115 | H089 — equal-instruction model switch | Avoid appending byte-identical model instructions | `core/src/context/world_state/model.rs::ModelInstructionsState::render_diff` content fingerprint |
| CF-116 | H113 — ambiguous transport completion and pre-first-commit crash | Actual lost-terminal replay; **provider contract required** for idempotent Remote V2 recovery | Persist a pending compaction operation identity before dispatch, reuse it across retries and process restart, and reconcile it through a provider-supported idempotency or completed-response contract in `core/src/compact_remote_v2.rs`; without that contract, durably quarantine ambiguous operations and do not resubmit automatically |
| CF-117 | #27 | Deduplicate realtime handoff text | `core/src/realtime_conversation.rs::realtime_delegation_from_handoff` model-facing projection |
| CF-118 | H043 | Bound aggregate effective realtime instructions | `prepare_realtime_start` / `build_realtime_session_config` typed bounded effective config |
| CF-119 | H044 | **Provider contract required:** Frameless partial-append replay risk; duplicate model-token commitment is unproven | Add service-recognized append ID/offset/ack before partial resume; retain full-replay behavior as a transport risk until commitment evidence exists |
| CF-120 | H045 | Skip assistant-only transcript-tail inference | `core/src/realtime_conversation.rs::flush_realtime_transcript_tail` role-aware admission |
| CF-121 | H046 — core-created Frameless | Close core-owned Frameless sessions explicitly | `realtime_conversation.rs::stop_conversation_state` with ownership-aware close handle |
| CF-122 | #17 | **Provider contract required:** Minimize the remote-compaction tool catalog | Keep the current catalog until the provider defines the valid referenced/minimum subset, then share that projection between `compact_remote_request.rs` and `compact_remote_v2_attempt.rs` |
| CF-123 | #18 | Rewrite older remote outputs past non-output separators | `core/src/compact_remote.rs::trim_function_call_history_to_fit_context_window` |
| CF-124 | H017 | Drop prior local summaries from V2 compaction input | `core/src/compact_remote_v2.rs::build_v2_compacted_history` content-kind rejection |
| CF-125 | H018 | Make pristine Remote V2 compaction a no-op | `core/src/compact_remote_v2_attempt.rs::run_remote_compact_v2_attempt` semantic preflight |
| CF-126 | H087 | **Provider contract required:** Charge legacy V1 compaction to rollout budgets | Extend `codex-api/src/endpoint/compact.rs::CompactHistoryResponse` with actual provider usage; absent that contract, use an explicitly approved conservative/error policy in `compact_remote.rs` and never fabricate usage |
| CF-127 | H115 | Expire completed coordination messages at V2 checkpoints | `compact_remote_v2.rs::is_retained_for_remote_compaction_v2` with durable task lifecycle metadata |

### Archived or non-actionable claims

| Source/facet | Disposition | Reason |
| --- | --- | --- |
| #4 — generic goal-continuation umbrella | Decomposed; no separate bug | Goal turns are intentional execution, not Unified Exec/Code Mode polling. Concrete continuation payload, interrupt, and accounting defects are CF-002, CF-030, and CF-034. |
| #10 | Retired policy claim | Uncapped Responses WebSocket recovery is intentional availability policy; a retry ceiling would not solve ambiguous-operation idempotency (CF-116). |
| #32 | Retired non-live headline | The inner HTTP and outer sampling retry factors do not compose on the production route at HEAD; retain provider replay semantics as future contract work. |
| H019 | Archived — Unlikely | The overwrite exists only on the false-by-default legacy SSE test path; active WebSocket returns semantic failures directly. |
| H046 — client-owned `ExistingCall` | Not a Core bug | Client-owned calls must not be forcibly closed; only core-created Frameless calls belong to CF-121. |
| H049 | Archived — Unlikely | Direct-parent result ownership is the documented collaboration contract; requester subscriptions would be a new feature. |
| H054 — `get_goal` retrieval | Not an unconditional defect | Returning the objective is the purpose of `get_goal`; only create/update need compact acknowledgements (CF-033). |
| H069 | Archived — Unlikely | Wire item IDs are not shown to affect rendered-token prompt-cache keys or cached-token usage. |
| H079 — high nonnegative override | Intentional policy | A large nonnegative user override deliberately raises retention; only negative signed catalog wrapping is a bug (CF-105). |
| H089 — clean first-turn override | Overstated/non-live | When startup instructions already match the override model, no model-switch fragment is emitted; equal-instruction later switches remain CF-115. |
| H104 | Archived optimization | Phase 2 terminal text is normally small and can be consumed by configured Stop hooks; use a minimal sentinel as CF-042/CF-043 acceptance criteria, not a separate bug. |
| H111 | Archived — Unlikely | Active-window tool evidence has not already been summarized and may be the only source the compactor must distill; only the narrower #18 control-flow defect is live (CF-123). |
| H114 — history rewrite/post-checkpoint branches | Expected full create | A rewritten pre-compaction history and the first request from client-built replacement history are not valid continuations; only the schema-only branch is CF-112. |
| H116 | Archived — Unlikely | Cross-model fallback follows a failed or rejected attempt; a valid completed compaction returns success and is not regenerated. |
| H117 | Archived — Unlikely | Tests prove tolerance of synthetic extra items, not production generation, billing, or a safe client-side prevention mechanism. |
| H099 — memory read-path prompt / CF-048 | Archived after canonical triage | Existing code rejects summaries above 64 KiB and truncates accepted summaries to 2,500 tokens; the remaining fixed guidance is an optimization question, not a live token defect. |
| H081 / CF-102 | Reclassified as non-token correctness | Reconstruction can lose a compacted prefix or leave an orphaned continuation, but additional token consumption depends on speculative later rediscovery or reruns. |
| H088 / CF-113 | Insufficient token evidence | The Copilot WebSocket preparation hook is not applied, but current evidence does not establish whether the service rejects, normalizes, or bills the unnormalized frame. |

### Source coverage map

Every source ID is mapped below. Facet labels are authoritative where a compound
source was split.

| Source | Canonical mapping or archive disposition |
| --- | --- |
| #1 | CF-011 |
| #2 | aggregate retained outputs -> CF-103; final per-output cap -> CF-019 |
| #3 | CF-017 |
| #4 | Unified Exec wait -> CF-064; Code Mode wait -> CF-065; agent wait -> CF-080; generic goal-continuation umbrella -> archived as decomposed/non-actionable |
| #5 | worker profile -> CF-001; stale cancellation -> CF-022 |
| #6 | CF-018 |
| #7 | CF-012 |
| #8 | CF-004 |
| #9 | CF-050 |
| #10 | archived — intentional WebSocket recovery policy |
| #11 | CF-010 |
| #12 | callback ownership -> CF-007; notification budget -> CF-072 |
| #13 | CF-057 |
| #14 | CF-044 |
| #15 | CF-017 |
| #16 | CF-021 |
| #17 | CF-122 |
| #18 | CF-123 |
| #19 | CF-107 |
| #20 | CF-108 |
| #21 | CF-082 |
| #22 | CF-081 |
| #23 | CF-074 |
| #24 | generic resource read -> CF-015; ordinary MCP result projection -> CF-016 |
| #25 | CF-020 |
| #26 | CF-030 |
| #27 | CF-117 |
| #28 | additional context -> CF-005; environment delta -> CF-054 |
| #29 | CF-098 |
| #30 | CF-029 |
| #31 | CF-008 |
| #32 | archived — non-live production 30-request headline |
| H001 | CF-023 |
| H002 | CF-024 |
| H003 | CF-025 |
| H004 | CF-026 |
| H005 | CF-051 |
| H006 | CF-052 |
| H007 | CF-004 |
| H008 | CF-066 |
| H009 | CF-067 |
| H010 | CF-068 |
| H011 | CF-069 |
| H012 | CF-083 |
| H013 | CF-088 |
| H014 | CF-008 |
| H015 | CF-089 |
| H016 | CF-021 |
| H017 | CF-124 |
| H018 | CF-125 |
| H019 | archived — Unlikely |
| H020 | CF-027 |
| H021 | CF-028 |
| H022 | CF-075 |
| H023 | CF-076 |
| H024 | CF-077 |
| H025 | CF-078 |
| H026 | CF-070 |
| H027 | CF-071 |
| H028 | CF-007 |
| H029 | CF-109 |
| H030 | CF-110 |
| H031 | CF-091 |
| H032 | CF-092 |
| H033 | CF-093 |
| H034 | CF-094 |
| H035 | CF-018 |
| H036 | CF-097 |
| H037 | CF-008 |
| H038 | CF-011 |
| H039 | CF-012 |
| H040 | CF-095 |
| H041 | CF-096 |
| H042 | CF-045 |
| H043 | CF-118 |
| H044 | CF-119 |
| H045 | CF-120 |
| H046 | core-created Frameless -> CF-121; client-owned `ExistingCall` -> archived as intended ownership |
| H047 | CF-079 |
| H048 | CF-009 |
| H049 | archived — Unlikely |
| H050 | CF-013 |
| H051 | CF-002 |
| H052 | CF-031 |
| H053 | CF-032 |
| H054 | create/update projection -> CF-033; `get_goal` retrieval -> archived as intentional |
| H055 | CF-034 |
| H056 | CF-035 |
| H057 | CF-001 |
| H058 | CF-019 |
| H059 | resource listing -> CF-014; resource read -> CF-015 |
| H060 | CF-014 |
| H061 | CF-046 |
| H062 | CF-014 |
| H063 | CF-016 |
| H064 | CF-020 |
| H065 | CF-106 |
| H066 | CF-017 |
| H067 | CF-104 |
| H068 | CF-013 |
| H069 | archived — Unlikely |
| H070 | CF-084 |
| H071 | CF-111 |
| H072 | CF-073 |
| H073 | CF-009 |
| H074 | complete context baseline -> CF-010; failed local compaction output -> CF-018; compound context/checkpoint persistence -> CF-099; interrupted-fork boundary -> CF-100 |
| H075 | CF-101 |
| H076 | CF-010 |
| H077 | CF-019 |
| H078 | CF-019 |
| H079 | negative catalog limit -> CF-105; high nonnegative override -> archived as intentional policy |
| H080 | CF-020 |
| H081 | CF-102 (archived from token backlog; non-token reconstruction correctness) |
| H082 | CF-006 |
| H083 | CF-005 |
| H084 | CF-006 |
| H085 | CF-055 |
| H086 | CF-056 |
| H087 | CF-126 |
| H088 | CF-113 (archived pending evidence of token impact) |
| H089 | equal-instruction model switch -> CF-115; clean first-turn override -> archived as overstated |
| H090 | CF-114 |
| H091 | CF-085 |
| H092 | CF-086 |
| H093 | same-turn aggregate -> CF-058; cross-turn repetition -> CF-059 |
| H094 | CF-060 |
| H095 | endpoint recommendations -> CF-061; legacy discovery -> CF-062 |
| H096 | CF-063 |
| H097 | DB-backed corpus -> CF-041; external-agent import -> CF-047 |
| H098 | CF-037 |
| H099 | Phase 1 prompt -> CF-039; Phase 2 prompt -> CF-042; memory read-path prompt -> CF-048 (archived; existing bounds cover the claim) |
| H100 | CF-043 |
| H101 | CF-038 |
| H102 | CF-040 |
| H103 | Phase 1 extraction -> CF-003; ordinary model history -> CF-049 |
| H104 | archived — non-actionable standalone optimization |
| H105 | CF-003 |
| H106 | CF-021 |
| H107 | CF-017 |
| H108 | CF-090 |
| H109 | CF-087 |
| H110 | repeated continuation reference -> CF-002; initial materialization -> CF-036 |
| H111 | archived — Unlikely |
| H112 | CF-053 |
| H113 | ambiguous completion/pre-first-commit crash -> CF-116; failed or partial checkpoint persistence -> CF-099 |
| H114 | schema-only pre-compaction -> CF-112; history rewrite/post-checkpoint -> archived as expected full-create behavior |
| H115 | CF-127 |
| H116 | archived — Unlikely |
| H117 | archived — Unlikely |

---

Original audit findings, hypothesis blocks, and adjudication evidence follow unchanged.

## 1. Tool catalogs are aggregate-unbounded and can dominate every request

- **Evidence:** `codex-rs/tools/src/mcp_tool.rs::parse_mcp_tool_with_description_limit`
  (`7-52`), `codex-rs/tools/src/json_schema.rs::compact_large_tool_schema`
  (`231-263`), `codex-rs/core/src/tools/handlers/mcp.rs::create_tool_spec`
  (`452-483`), `codex-rs/app-server/src/request_processors/thread_processor.rs::validate_dynamic_tools`
  (`281-425`), `codex-rs/codex-mcp/src/pagination.rs::collect_paginated_with_limit`
  (`9-66`), `codex-rs/core/src/tools/spec_plan.rs::build_model_visible_specs`
  (`496-530`), and `codex-rs/core/src/client.rs::build_responses_request`
  (`895-932`).
- **Mechanism:** Ordinary MCP descriptions are not hard-capped, schema compaction is
  explicitly best-effort, namespace descriptions allow 512 KiB each, dynamic tools
  have no count/description/aggregate limit, and per-server item limits have no
  cross-server byte budget. The complete resulting catalog is model-visible on every
  sampling request. The default `gpt-5.6-sol` CodeModeOnly path additionally
  concatenates all nested declarations into one uncapped `exec` description in
  `codex-rs/code-mode-protocol/src/description.rs::build_exec_tool_description`
  (`261-339`).
- **Waste:** Per-request, multiplicative by tools, servers, turns, retries, and child
  agents.
- **Impact:** A measured 2,048-tool wide-schema catalog serialized to about 66 MiB per
  request; 32 maximum-sized namespace descriptions add about 16 MiB. A plausible
  100-tool dynamic catalog can exceed 100K input tokens. The code-level upper bound is
  effectively unlimited.
- **Confidence:** confirmed.

## 2. Tool-output limits multiply per item, and the nominal 10K cap is actually 12K

- **Evidence:** `codex-rs/core/src/context_manager/history.rs::process_item`
  (`470-508`) applies `policy * 1.2` independently to every function/custom-tool
  output; `record_items_with_metadata` (`186-200`) retains every processed item; the
  next request clones all history in `codex-rs/core/src/session/turn.rs` (`371-390`).
- **Mechanism:** A 10K-token model policy admits about 12K content tokens before JSON
  framing. There is no aggregate budget across serial or parallel outputs, so every
  result receives the full allowance and every later request carries all retained
  results until compaction.
- **Waste:** Cumulative and multiplicative; approximately `L * N` active output context
  and `L * N * (N + 1) / 2` cumulative input across serial follow-ups.
- **Impact:** At the effective 12K limit, 15 outputs can retain about 180K tokens and
  contribute about 1.44M cumulative request-input tokens before cache discounts.
  Parallel waves can cross the context threshold in one step and add a compaction call.
- **Confidence:** confirmed.

## 3. Context-fit accounting omits catalogs, output schemas, and other final request fields

- **Evidence:** `codex-rs/core/src/context_manager/history.rs::estimate_token_count_with_base_instructions`
  (`269-282`) counts base instructions and history only. Actual construction later adds
  Responses Lite `AdditionalTools`, ordinary `tools`, and structured-output schema in
  `codex-rs/core/src/client.rs::build_responses_request` (`904-984`). Remote compaction
  repeats the mismatch in `codex-rs/core/src/compact_remote.rs`
  (`399-424`), `compact_remote_request.rs` (`62-74`), and
  `compact_remote_v2_attempt.rs` (`74-84`).
- **Mechanism:** Startup, resume, post-compaction, catalog-change, structured-output,
  and compaction admission checks can declare a request within budget even though its
  serialized model context is over the window. Compaction cannot reduce an immutable
  oversized catalog, so a failed request can be followed by a useless compaction and
  another failure.
- **Waste:** Retry-triggered, cumulative, and multiplicative by catalog size.
- **Impact:** A best-effort 5,000-byte schema is roughly 1,250 heuristic tokens before
  descriptions and framing. About 11 such schemas consume the local 5% reserve for a
  272K model; about 22 consume the default 10% auto-compaction headroom.
- **Confidence:** confirmed.

## 4. Long-running operations are polled through repeated full model turns

- **Evidence:** Unified Exec defaults and clamps in
  `codex-rs/core/src/tools/handlers/unified_exec.rs` (`36, 60-66`) and
  `core/src/unified_exec/process_manager.rs` (`887-899`); Code Mode yields after 10
  seconds and gives each `wait` a fresh output budget in
  `code-mode-protocol/src/runtime.rs` (`11-13`) and
  `core/src/tools/code_mode/wait_handler.rs` (`24-32, 155-159`); agent wait defaults to
  30 seconds in `core/src/tools/handlers/multi_agents_common.rs` (`28-31`); goal waits
  immediately continue through `ext/goal/src/runtime.rs` (`363-417`).
- **Mechanism:** A timeout/status result is persisted as tool output. Continuing to wait
  requires another inference to decide to issue the same poll. There is no shared
  host-side durable wait or completion-triggered continuation for these paths.
- **Waste:** Background, cumulative, and extra-call polling loops.
- **Impact:** A quiet five-minute Unified Exec command can require roughly 58 model
  responses with defaults. Code Mode and agent waits add one full inference per yield
  or 30-second timeout; each poll can also add another independently capped output.
- **Confidence:** confirmed.

## 5. Tiny title and recap jobs inherit the full coding-agent context, and stale jobs keep running

- **Evidence:** `codex-rs/tui/src/temporary_structured_request.rs::start_temporary_structured_thread`
  (`60-148`) disables tools but starts a root-like feature thread. Root instructions
  load in `core/src/thread_manager.rs::user_instructions_for_spawn` (`1609-1621`);
  AGENTS discovery runs in `core/src/session/session.rs` (`1181-1205`) and enters world
  state via `core/src/session/world_state.rs` (`150`). Title and recap payloads are only
  960 and 900 bytes in `tui/src/app/thread_title.rs` (`24, 229-239`) and
  `tui/src/app/recap.rs` (`52, 108`).
- **Mechanism:** Metadata inference pays normal base, user, project, and world-state
  instructions despite needing only a tiny bounded prompt. A 30-second timeout merely
  unsubscribes in `temporary_structured_request.rs` (`278-293`);
  `thread/unsubscribe` does not interrupt the turn
  (`app-server/src/request_processors/thread_processor.rs:1010-1035`). Focus, thread,
  title, or revision changes can discard the eventual result while generation
  continues, and recap retry can overlap the original.
- **Waste:** Background, per-call, and retry-multiplicative.
- **Impact:** This repository's root `AGENTS.md` alone is about 32.4 KiB, plus base and
  world-state context, for each title/recap call. A stale or timed-out call wastes the
  entire invocation; an overlapping retry can double it.
- **Confidence:** confirmed.

## 6. Local compaction removes one oldest item per rejected full resubmission

- **Evidence:** `codex-rs/core/src/compact.rs::run_compact_task_inner_impl`
  (`274-330`) handles each context error by calling
  `ContextManager::remove_first_item`, resetting retries, and resubmitting.
  `core/src/context_manager/history.rs` (`285-297`) removes exactly one item or its
  paired call/output group. `core/tests/suite/compact.rs` (`3638-3752`) asserts that the
  retry is exactly one item shorter.
- **Mechanism:** If several items must be removed, each size is discovered through
  another complete compaction model request. A shortened failed request cannot use
  WebSocket `previous_response_id`, and resetting retries gives each shortened size a
  fresh transport-retry allowance.
- **Waste:** Retry-triggered and multiplicative.
- **Impact:** If `k` removals are required, `k` rejected compaction calls precede
  success. With many similarly sized items, submitted input approaches quadratic
  growth in item count.
- **Confidence:** confirmed.

## 7. Deferred tool search retains duplicate 32 KiB schema batches and can compact them before use

- **Evidence:** `codex-rs/tools/src/tool_discovery.rs::bound_tool_search_output`
  (`7-65`) caps each result independently at 32 leaves/32 KiB.
  `core/src/session/turn.rs` (`2155-2168`) and
  `core/src/context_manager/history.rs` (`510-523`) append every result without
  query/schema deduplication. `core/tests/suite/search_tool.rs` (`1744-1845`) verifies
  five searches returning the same dynamic schema five times in the next request.
- **Mechanism:** Separate searches accumulate duplicate tool definitions. If their
  aggregate crosses the threshold, the post-tool check compacts before the required
  follow-up (`core/src/session/turn.rs:425-499`), while local and remote compaction
  discard or empty `ToolSearchOutput` (`core/src/compact.rs:354-380`,
  `compact_remote.rs:370-400, 489-503`). The model can then search again for data it
  never consumed.
- **Waste:** Cumulative, multiplicative, compaction-triggered, and extra-call.
- **Impact:** One full result is roughly 8.2K heuristic tokens. Five duplicate full
  results can add about 41K tokens before envelopes, plus a compaction call and a
  possible rediscovery round.
- **Confidence:** confirmed.

## 8. Hook additional context has no aggregate bound and can explicitly be unlimited

- **Evidence:** `codex-rs/config/src/hook_config.rs` (`164-177`) accepts arbitrary
  `additionalContextLimit`; `codex-rs/hooks/src/output_spill.rs`
  (`19-24, 64-72`) treats zero as no spilling. Tests preserve outputs above 10K for
  zero and `usize::MAX` (`hooks/src/output_spill_tests.rs:52-82`).
  `maybe_spill_additional_contexts` (`93-118`) applies limits independently to every
  fragment, and `core/src/hook_runtime.rs` (`764-783`) records every resulting
  developer message.
- **Mechanism:** `N` matching handlers can each inject the full limit on session,
  prompt, pre-tool, and post-tool events. Pre-turn compaction occurs before prompt-hook
  output is added, with no final aggregate size gate. Async hook completions can arrive
  in waves and each wave can make pending input force another sampling request
  (`core/src/session/turn.rs:410-425`).
- **Waste:** Per-request, cumulative, multiplicative, and background.
- **Impact:** The default permits `N * 2,500` tokens per hook event; configured zero or
  very large limits make the upper bound unlimited. Repeated tool hooks multiply this
  across a turn.
- **Confidence:** confirmed.

## 9. Stop hooks can force an unbounded sampling loop while retaining every continuation prompt

- **Evidence:** `codex-rs/core/src/session/turn.rs` (`502-537`) appends a blocking
  continuation prompt and immediately samples again. `stop_hook_active` is passed to
  the hook but is not an enforced ceiling. `core/tests/suite/hooks.rs`
  (`1300-1406`) verifies three requests in one turn and cumulative retention of the
  earlier hook prompts.
- **Mechanism:** A hook that continues to block produces another model call and another
  persistent user-role prompt after each otherwise complete response.
- **Waste:** Background, cumulative, and multiplicative.
- **Impact:** Unbounded model invocations and monotonically growing input until
  cancellation or external hook behavior changes.
- **Confidence:** confirmed. This is distinct from the fixed memory-worker Stop-hook
  termination issue.

## 10. WebSocket retry handling can replay full context without a retry ceiling

- **Evidence:** `codex-rs/codex-api/src/sse/responses.rs` (`503-522`) classifies
  `response.failed` with `error: null` as retryable.
  `core/src/responses_retry.rs` (`81-111`) increments WebSocket retries but never
  compares them with `max_retries`. The failed socket is discarded in
  `codex-api/src/endpoint/responses_websocket.rs` (`325-331`), so no completed
  `previous_response_id` is available. `core/tests/suite/websocket_retry.rs`
  (`137-181`) verifies a new handshake with identical nonempty full input.
- **Mechanism:** Sampling retries rebuild complete history indefinitely. Remote
  compaction v2 declares a two-retry cap in `core/src/compact_remote_v2.rs`
  (`374-416`) but calls the same uncapped WebSocket branch.
- **Waste:** Retry-triggered and multiplicative.
- **Impact:** Unbounded full-context sampling or compaction invocations for a persistent
  retryable terminal event.
- **Confidence:** confirmed. `response.incomplete` is correctly terminal and is not
  included.

## 11. Nested full-history forks can duplicate the complete initial context bundle

- **Evidence:** Full forks retain `TurnContext` and `WorldState` in
  `core/src/agent/control/spawn.rs::keep_forked_rollout_item` (`63-100`), but
  `core/src/session/rollout_reconstruction.rs::finalize_active_segment`
  (`107-135`) restores the reference baseline only for a surviving user-turn boundary
  or explicit clear. The child then sees no baseline and calls
  `build_initial_context_with_world_state` again in
  `core/src/session/mod.rs::record_context_updates_and_set_reference_context_item`
  (`3995-4041`).
- **Mechanism:** The fork retains the parent's initial developer/environment context,
  then the child's first request appends another complete copy because the surviving
  full world-state snapshot is not treated as a baseline.
- **Waste:** Background and multiplicative by descendant depth.
- **Impact:** One full initial-context duplicate per affected child; nested forks repeat
  it. Upstream commit `e42b66ad9d` contains the targeted fix and request-level
  regression coverage, but is not an ancestor of this worktree.
- **Confidence:** confirmed.

## 12. Code Mode notifications can inject unlimited stale output into later turns

- **Evidence:** `codex-rs/code-mode-runtime/src/runtime/callbacks.rs::notify_callback`
  (`265-290`) accepts any nonempty string without size/count limits.
  `core/src/tools/code_mode/delegate.rs::CodeModeDispatchBroker::new`
  (`41-46`) uses an unbounded channel. Per-request workers consume that shared queue
  (`101-146`) and `CoreTurnHost::notify` (`374-388`) injects raw text through
  `core/src/session/inject.rs::inject_if_running` (`15-30`).
- **Mechanism:** A background cell can queue arbitrary notifications after its original
  request worker exits. A later turn's worker can consume them with the new turn host;
  pending input then forces another inference.
- **Waste:** Background, cumulative, and extra-call.
- **Impact:** Unbounded notification bytes and potentially unbounded additional
  sampling iterations.
- **Confidence:** confirmed.

## 13. `skills.read` can turn one 1 MiB skill into many model calls and triangular context growth

- **Evidence:** Skill instructions require reading a selected skill completely and
  paginating to EOF (`codex-rs/ext/skills/src/catalog_prompt.rs:11, 28`).
  Providers allow 1 MiB resources (`ext/skills/src/provider.rs:30`), while
  `ext/skills/src/tools/read.rs` (`76, 254`) pages using the ordinary output policy.
  Exact repeated reads are not output-deduplicated
  (`core/tests/suite/skills_extension.rs:1107, 1156-1173`).
- **Mechanism:** Each page becomes retained tool output and requires another inference
  to request the next page. Earlier pages are resent on every later page request.
  Explicit activation already caps the main prompt at 8,000 bytes
  (`ext/skills/src/render.rs:19, 1176-1177`), showing that complete 1 MiB exposure is
  not required by normal activation semantics.
- **Waste:** Background, cumulative, and multiplicative.
- **Impact:** At the effective roughly 12K-token page allowance, a 1 MiB skill can
  require about 22 follow-up model iterations, hundreds of thousands of output tokens,
  and much larger cumulative input.
- **Confidence:** confirmed; occurrence is conditional on a large skill.

## 14. Memory consolidation's exhausted retry counter does not stop future model retries

- **Evidence:** `codex-rs/state/src/runtime/memories.rs::mark_global_phase2_job_failed`
  (`1295-1331`) decrements `retry_remaining`. The claim path
  `try_claim_global_phase2_job` (`1076-1219`) neither selects nor checks that field;
  after `retry_at` expires it can mark the job running again. Startup invokes phase 2
  from `codex-rs/memories/write/src/start.rs` (`24-80`).
- **Mechanism:** A persistent consolidation failure can be reclaimed after each
  one-hour delay even after the nominal three retries reach zero, launching another
  expensive Terra worker on later ordinary turns.
- **Waste:** Background and retry-triggered.
- **Impact:** Unbounded consolidation model attempts over the lifetime of the failed
  job. The feature is currently default-disabled, but the active path has no effective
  retry ceiling.
- **Confidence:** confirmed. This is distinct from the fixed memory Stop-hook issue.

## 15. First-request admission ignores all newly arriving context

- **Evidence:** `codex-rs/core/src/session/turn.rs` (`159-171`) explicitly runs
  pre-sampling compaction before recording context updates and user input. World-state
  changes, hooks, user input, skill/plugin injections, tools, and output schema are
  added later (`227, 268, 280, 1332-1340`). A provider context rejection marks the
  window full and returns instead of compacting/retrying in place (`1412-1425`).
- **Mechanism:** A near-threshold session can admit a large turn using only its old
  usage estimate, send an over-window request, and require a later compaction plus
  caller resubmission. Media is especially exposed: app-server aggregate validation
  counts image/audio as zero characters
  (`app-server-protocol/src/protocol/v2/turn.rs:473-482`).
- **Waste:** Per-request, retry-triggered, and multiplicative for repeated media.
- **Impact:** Text input alone permits 1,048,576 characters, roughly 262K heuristic
  tokens. Images/audio have no aggregate count/token limit, so the upper request cost
  is not bounded by the admission check.
- **Confidence:** confirmed.

## 16. Remote compaction v2 retains audio at zero budget cost

- **Evidence:** `codex-rs/core/src/compact_remote_v2_images.rs::content_item_token_count`
  (`15-30`) and `core/src/compact_remote_v2.rs::message_text_token_count`
  (`689-702`) assign `InputAudio` zero. Truncation preserves audio after text budget is
  exhausted (`compact_remote_v2_images.rs:75-91`,
  `compact_remote_v2.rs:705-733`). Normal history estimation charges audio by duration
  (`core/src/context_manager/history.rs:673-683, 909-943`).
- **Mechanism:** Old audio is sent to the compaction model, survives the nominal 64K
  retained-message budget for free, and is sent again in subsequent inference
  requests and after resume.
- **Waste:** Cumulative and multiplicative.
- **Impact:** Audio tokens are paid during compaction and every later request. Enough
  retained audio can leave the post-compaction context above threshold and trigger
  repeated compaction.
- **Confidence:** confirmed. This is distinct from the fixed old-image retention issue.

## 17. Remote compaction sends the complete tool catalog even though it cannot execute tools

- **Evidence:** V1 uses `tool_router.model_visible_specs()` in
  `codex-rs/core/src/compact_remote_request.rs` (`62-74`); v2 does the same and enables
  parallel calls in `core/src/compact_remote_v2_attempt.rs` (`74-84`).
  `core/src/client.rs::build_responses_request` (`925-985`) serializes the catalog with
  automatic tool choice. V2 collection consumes only the compaction item and ignores
  other output items (`core/src/compact_remote_v2.rs:419-472`).
- **Mechanism:** Every compact attempt pays for all tool names, descriptions, schemas,
  and namespace text. The compaction path has no tool-dispatch loop, so any emitted tool
  call is also paid for and discarded.
- **Waste:** Per-compaction and multiplicative on retry/fallback.
- **Impact:** One full catalog per attempt; with the unbounded catalogs in finding 1,
  this can mean thousands to hundreds of thousands of extra input tokens per compact.
- **Confidence:** likely. Transmission is confirmed; the external compaction contract
  does not document whether any schemas are required.

## 18. Remote compaction v1 stops output trimming at the newest non-output item

- **Evidence:** `codex-rs/core/src/compact_remote.rs::trim_function_call_history_to_fit_context_window`
  (`399-454`) scans newest-to-oldest but executes `break` when a group is not
  rewritable (`431-437`). Only function/custom/search outputs are rewritable
  (`457-510`).
- **Mechanism:** A newest user, assistant, reasoning, call, or compaction item prevents
  traversal from reaching every older oversized tool output. Those outputs then enter
  the compaction request unchanged and can cause rejection or fallback.
- **Waste:** Per-request, retry-triggered, and multiplicative by skipped outputs.
- **Impact:** At least one and potentially all older outputs remain; several 10K-token
  outputs can add tens of thousands of avoidable tokens.
- **Confidence:** confirmed.

## 19. Responses Lite places the rebuilt tool catalog at input item zero, invalidating the whole cacheable suffix

- **Evidence:** `codex-rs/core/src/client.rs::build_responses_request` (`894-927`)
  reconstructs complete `AdditionalTools` at `input[0]` and hashes the full catalog
  into its item ID. Tool mutations arise from MCP, extensions, hosted availability, and
  dynamic tools in `core/src/tools/spec_plan.rs`.
- **Mechanism:** Any tool name, description, schema, availability, or order change
  changes the first rendered item, so unchanged base instructions and conversation
  history after it cannot match the prior cached prefix. Current
  [OpenAI prompt-caching guidance](https://developers.openai.com/api/docs/guides/prompt-caching)
  recommends stable definitions and append-only `additional_tools` updates.
- **Waste:** Cumulative cache-write and uncached-input amplification.
- **Impact:** Potentially the entire otherwise stable prompt prefix must be processed
  and cache-written again after each catalog mutation. GPT-5.6 cache writes are billed
  above ordinary uncached input and later cache reads are heavily discounted, making
  repeated invalidation material.
- **Confidence:** likely; code placement and provider prefix semantics are confirmed,
  while realized misses depend on actual catalog changes and routing.

## 20. GPT-5.6 compaction cannot preserve an explicit cache breakpoint after stable instructions

- **Evidence:** `codex-rs/codex-api/src/common.rs::ResponsesApiRequest` (`302-370`)
  exposes `prompt_cache_key` but no `prompt_cache_options` or content-level
  `prompt_cache_breakpoint`. Local and remote compaction replace history in
  `core/src/compact.rs` (`344-389`) and
  `core/src/compact_remote_v2.rs` (`309-344`).
- **Mechanism:** Compaction changes the rendered context from the replacement point
  onward. GPT-5.6 supports an
  [explicit breakpoint](https://developers.openai.com/api/docs/guides/prompt-caching)
  after stable developer content, but Codex cannot express one, so the next request
  cannot deliberately reuse only the unchanged tools/base-instruction prefix and must
  rely on a new implicit cache write.
- **Waste:** Cumulative prompt-cache loss at every compaction checkpoint.
- **Impact:** The stable instruction/catalog prefix is freshly processed and potentially
  cache-written after each compaction; savings scale with prefix size and remaining
  turns in the thread.
- **Confidence:** likely; current provider behavior is documented, but exact realized
  savings depend on cache eligibility and routing.

## 21. Inline review output is persisted twice in the parent model context

- **Evidence:** `codex-rs/core/src/tasks/review.rs::exit_review_mode`
  (`217-260`) renders the same explanation/findings as both an XML-wrapped synthetic
  user message and a plain assistant message, then records both.
  `core/tests/suite/review.rs` (`242-287`) verifies both copies in the rollout.
- **Mechanism:** Subsequent parent requests include both review representations until
  compaction even though they convey substantially the same findings.
- **Waste:** Cumulative and multiplicative by remaining parent turns.
- **Impact:** If review output is `R` tokens, approximately another `R` plus XML framing
  is retained and resent.
- **Confidence:** confirmed.

## 22. Detached review processes the complete parent history

- **Evidence:** `codex-rs/app-server/src/request_processors/turn_processor.rs`
  (`1465-1494`) explicitly forks full parent history for detached review;
  `codex-rs/core/src/thread_manager.rs` (`988-1014`) loads it with
  `include_history: true`. The detached prompt separately loads the review skill and
  target (`turn_processor.rs:1585-1598`).
- **Mechanism:** A review worker that can inspect its target directly still processes
  every retained parent message, tool result, and reasoning item or compacts before its
  first useful request. Inline review starts without initial history.
- **Waste:** Background and per-review.
- **Impact:** From a small prefix to nearly an entire context window, plus a possible
  compaction call.
- **Confidence:** likely. Full-history transmission is confirmed; the avoidable share
  depends on how much conversational context the chosen review target actually needs.

## 23. Agent status APIs repeatedly return completed result bodies

- **Evidence:** V1 `wait_agent` returns immediately if any target is final in
  `codex-rs/core/src/tools/handlers/multi_agents/wait.rs` (`115-153`) and includes the
  completed assistant message from `core/src/agent/status.rs` (`6-12`). It has no
  acknowledgment/cursor, while completion notification independently forwards the same
  status (`core/src/agent/control.rs:565-657`). V2 `ListedAgent` embeds full
  `AgentStatus` (`core/src/agent/control.rs:103-107`), and
  `list_agents` returns every live completed body (`493-560`).
- **Mechanism:** Repeated V1 waits can replay one completed result and starve still
  running siblings; repeated V2 lists resend all completed results. Every snapshot is
  retained as another tool output.
- **Waste:** Cumulative and multiplicative by completed agents and polling calls.
- **Impact:** `completed-result bytes * completed agents * list/wait calls`, plus extra
  parent inferences required to keep waiting.
- **Confidence:** confirmed.

## 24. MCP embedded resources and binary reads are converted to model-visible JSON text

- **Evidence:** `codex-rs/protocol/src/models.rs::convert_mcp_content_to_items`
  (`2292-2379`) handles only text/image/audio explicitly; embedded `resource` and
  `resource_link` blocks fall through to `serde_json::to_string`.
  `core/src/tools/handlers/mcp_resource.rs::serialize_function_output`
  (`280-362`) serializes the entire `ReadResourcePayload`, including raw blob contents,
  server, and repeated URI, then truncates the text.
- **Mechanism:** Embedded image/audio base64, icons, annotations, `_meta`, and resource
  wrappers become expensive text rather than typed media or a bounded descriptor.
  The top-level unsupported-media sanitizer cannot see media nested inside that JSON.
- **Waste:** Per-call and cumulative until compaction.
- **Impact:** Each block/read can consume the full tool-output budget with largely
  unusable base64 text; multiple resources multiply it.
- **Confidence:** confirmed. This is distinct from the fixed MCP result
  double-encoding issue.

## 25. Resume can re-expand old tool outputs under the current model's larger policy

- **Evidence:** Live history stores a processed copy in
  `codex-rs/core/src/session/mod.rs` (`3200-3225`), but rollout persistence receives the
  original prepared envelope. Reconstruction retruncates uncompacted suffix outputs
  using the current model policy in
  `core/src/session/rollout_reconstruction.rs` (`370-410`).
- **Mechanism:** Switching from a byte-limited model to a token-limited model can make
  old raw output much larger in the first resumed request than it was in the live
  context that produced the rollout.
- **Waste:** Cumulative and resume-triggered.
- **Impact:** An output previously retained at roughly 12 KiB can reappear at up to
  about 12K tokens; multiple outputs can force immediate pre-turn compaction.
- **Confidence:** confirmed. Compacted replacement history is replayed verbatim and is
  not affected.

## 26. Interrupting an active goal can immediately launch a replacement model turn

- **Evidence:** Interrupted turns report `ThreadIdleCause::Interrupted`
  (`codex-rs/core/src/tasks/mod.rs:796-804, 856-857`). Goal abort accounting leaves an
  active goal active (`ext/goal/src/accounting.rs:428-438`), and
  `ext/goal/src/extension.rs::on_thread_idle` (`148-154`) ignores the idle cause and
  calls `continue_if_idle`, which starts another turn (`ext/goal/src/runtime.rs:363-417`).
- **Mechanism:** Direct app-server `turn/interrupt` can be followed immediately by a
  goal-triggered inference. The TUI separately pauses the goal, but other clients do
  not get that safeguard.
- **Waste:** Extra-call and cancellation-defeating.
- **Impact:** One unwanted full inference after each interrupt; repeated cancellation
  attempts can repeatedly relaunch work.
- **Confidence:** confirmed. This is distinct from the fixed post-completion goal loop.

## 27. Realtime handoff duplicates the same user text in two fields

- **Evidence:** The realtime collector ensures `input_transcript` is present in
  `active_transcript` (`codex-rs/codex-api/src/endpoint/realtime_websocket/methods.rs:
  640-641, 747`). `core/src/realtime_conversation.rs`
  (`1647-1671`) then renders it once as `<input>` and again inside
  `<transcript_delta>`. `core/tests/suite/realtime_conversation.rs`
  (`5277-5278`) asserts the exact duplication.
- **Mechanism:** The ordinary delegated inference receives identical user text twice in
  one model-visible message.
- **Waste:** Per-delegation and cumulative.
- **Impact:** Each field has an independent 4 KiB cap, so nearly 4 KiB, about 1K
  tokens, can be redundant per handoff.
- **Confidence:** confirmed.

## 28. Current-state fragments can append identical or mostly unchanged model-visible snapshots

- **Evidence:** `codex-rs/core/src/state/additional_context.rs::prepare`
  (`54-84, 108-113`) fingerprints the full raw value before rendering.
  Rendering truncates to 1,000 tokens in
  `context-fragments/src/additional_context.rs` (`94-101`).
  `core/src/state/additional_context_tests.rs` (`58-66`) proves two different raw
  values can produce identical model-visible items and still be republished.
  Environment diffs emit date, timezone, network, filesystem, and subagent fields
  together in `core/src/context/world_state/environment.rs` (`113-151`).
- **Mechanism:** Changes only in a truncated-away portion append another identical
  1,000-token fragment. An unrelated environment-field change appends unchanged
  filesystem/network/subagent text along with the changed field.
- **Waste:** Cumulative and multiplicative by updates.
- **Impact:** About 1,000 duplicate tokens per oversized additional-context
  publication, plus potentially large repeated environment projections until
  compaction.
- **Confidence:** confirmed.

## 29. Local compaction summarizes user instructions and then retains them verbatim

- **Evidence:** The summary prompt asks for important context, constraints, and user
  preferences (`codex-rs/core/prompts/templates/compact/prompt.md:1-9`).
  `core/src/compact.rs` collects real user messages (`527-570`), retains up to 20K
  tokens of them, and appends the summary separately (`650-732`).
  `core/tests/suite/compact.rs` (`1398-1629, 1789-1824`) verifies original user text and
  the summary coexist across repeated compactions.
- **Mechanism:** The generated summary restates task constraints that are then present
  verbatim as separate retained messages. Both are input to later requests and to the
  next compaction.
- **Waste:** Cumulative and multiplicative after compaction.
- **Impact:** From a few repeated constraint tokens to a substantial fraction of the
  20K retained-user-message allowance on every later request.
- **Confidence:** likely; structural coexistence is confirmed, while semantic overlap
  depends on the generated summary.

## 30. Current-time and rollout-budget reminders retain superseded values

- **Evidence:** Persistent reasoning enables the time reminder in
  `codex-rs/core/src/session/turn_context.rs` (`741-743`), whose default interval is
  one second (`core/src/config/mod.rs:1240-1246`). It runs before sampling and appends
  another history item (`core/src/session/time_reminder.rs:79-90, 133-136`).
  `core/tests/suite/current_time_reminder.rs` (`185-233`) verifies old and new
  timestamps coexist. Rollout-budget thresholds append similarly in
  `core/src/rollout_budget.rs` (`67-90`), and
  `core/tests/suite/rollout_budget.rs` (`61-120`) verifies old and new remaining-token
  values coexist.
- **Mechanism:** Only the latest timestamp or remaining-budget value describes current
  state, but every obsolete value remains in subsequent requests until compaction.
- **Waste:** Cumulative per inference/threshold.
- **Impact:** Small per item but high cadence in tool-heavy persistent-reasoning
  sessions; rollout reminder count is configuration-unbounded.
- **Confidence:** confirmed.

## 31. Synchronous Guardian generates reasoning summaries that it never consumes

- **Evidence:** Guardian inherits the parent reasoning-summary setting in
  `codex-rs/core/src/guardian/review.rs` (`983`) and applies it in
  `core/src/guardian/review_session.rs` (`1168`). The request asks for the summary in
  `core/src/client.rs` (`865-882`) while separately retaining encrypted reasoning
  (`953`). Guardian's event loop ignores reasoning events and consumes only completion
  and final JSON (`review_session.rs:1315-1344`).
- **Mechanism:** Plaintext reasoning summaries cost output tokens, then remain input in
  the reusable Guardian history even though the reviewer has no consumer for them and
  encrypted reasoning already provides continuity.
- **Waste:** Background output plus cumulative input.
- **Impact:** Usually tens to hundreds of tokens per approval, larger with detailed
  summaries and parse retries.
- **Confidence:** confirmed when the selected model supports summaries and the effective
  setting is not `none`.

## 32. HTTP-enabled inference stacks retry budgets and can send 30 complete requests

- **Evidence:** `codex-rs/codex-client/src/retry.rs` (`21-35, 89-101`) performs five
  total HTTP attempts by default. Generic 5xx responses remain retryable through
  `codex-api/src/api_bridge.rs` (`143-194`) and
  `protocol/src/error.rs` (`408-415`), after which the outer sampling loop in
  `core/src/session/turn.rs` (`1380-1462`) can start six complete HTTP batches.
- **Mechanism:** Persistent 5xx or transport failures exhaust the inner request budget,
  become a retryable stream error, and reopen another full batch. `Retry-After` and
  overload body semantics are parsed too late, and unlisted terminal
  `response.failed` types default to retryable.
- **Waste:** Retry-triggered and multiplicative.
- **Impact:** Default theoretical ceiling of `5 * 6 = 30` full POST bodies; overload
  handling alone can send five bodies after the first response already reports model
  unavailability.
- **Confidence:** conditional. The current ordinary production route is WebSocket-only,
  but this remains active for HTTP-enabled/public client paths.


# Max-Effort Second-Pass Hypotheses (Fleet Adjudicated)

These are the original agent-reported hypotheses from the repeated 32-lane audit.
All 117 later received an independent fleet verdict of **Plausible** or **Unlikely**;
the adjudication tables immediately below are authoritative. `Agent confidence` in
each preserved hypothesis block is the original reporter's label, not the final verdict.
Exact duplicate reports across lanes were merged during aggregation.

**Total unique hypotheses:** 117

## Hypotheses by lane

| Lane | Count |
| --- | ---: |
| `agent-status-paths` | 5 |
| `automations` | 4 |
| `code-mode` | 3 |
| `context-estimation` | 2 |
| `context-reminders` | 2 |
| `deferred-tool-search` | 5 |
| `dynamic-tools` | 1 |
| `goals` | 6 |
| `history-normalization` | 1 |
| `hooks` | 4 |
| `http-recovery` | 1 |
| `images-multimodal` | 4 |
| `local-compaction` | 3 |
| `mcp-discovery` | 4 |
| `mcp-results-resources` | 8 |
| `mcp-tool-schemas` | 1 |
| `memory-workers` | 8 |
| `misc-model-calls` | 2 |
| `parallel-tools` | 3 |
| `prompt-cache` | 1 |
| `realtime` | 4 |
| `reasoning-reviewers` | 5 |
| `remote-compaction-v1` | 1 |
| `remote-compaction-v2` | 8 |
| `request-construction` | 5 |
| `rollout-resume` | 4 |
| `skills-plugins-apps` | 4 |
| `subagent-forwarding` | 3 |
| `tool-output-retention` | 4 |
| `unified-exec` | 4 |
| `websocket-recovery` | 2 |
| `world-state` | 5 |

## Aggregated hypotheses

### Fleet confidence adjudication (2026-08-29)

H001-H117 were each independently reviewed by one `gpt-5.6-sol-fast` agent at `max` reasoning against the current code. These verdicts adjudicate the original unverified `Agent confidence` labels.

- **Plausible: 111** — current code supports a reachable mechanism under the stated conditions.
- **Unlikely: 6** — current code or intended behavior materially undercuts the original claim.

#### H001-H050

| Hypothesis | Verdict | Justification |
| --- | --- | --- |
| H001 | **Plausible** | The exercised automation path starts a fresh thread with only generic `thread_source = "automation"` and per-run metadata, while the stable scheduled-task `key` exists separately (`app-server/tests/suite/v2/client_metadata.rs:66-97`; `app-server-protocol/src/protocol/v2/plugin.rs:730-734`). A fresh root start generates a new thread ID and adopts it as the session ID (`core/src/session/session.rs:732-766`), and ordinary Responses requests use that session ID as `prompt_cache_key` (`core/src/client.rs:514-525`, `core/src/client.rs:970-985`), so thread-per-recurrence hosts cannot preserve task-level key affinity. Each new session also reloads AGENTS content (`core/src/session/session.rs:1181-1205`), but actual waste requires an identical cacheable prefix, a supported provider/model, an unexpired entry, and routing conditions where the key matters. |
| H002 | **Plausible** | `responsesapiClientMetadata` is explicitly outbound request metadata, and Core merely stores it on the active turn; `clientUserMessageId` is copied onto emitted user-message items, with neither value consulted before routing (`app-server-protocol/src/protocol/v2/turn.rs:153-173`, `core/src/session/turn_input.rs:264-318,601-621`, `core/src/session/mod.rs:4219-4237`). Replayed `turn/start` requests are serialized per thread, then steer an active regular turn or start a fresh one; pending steer input forces follow-up sampling, and the integration test records two model requests (`app-server-protocol/src/protocol/common.rs:967-983`, `core/src/session/turn.rs:307-320,413-425`, `app-server/tests/suite/v2/turn_start.rs:315-410`). Queue adds pass the client ID straight into an insert with a fresh UUID and no client-ID uniqueness until the 100-item pending cap (`app-server/src/request_processors/thread_queue_processor.rs:72-88`, `state/src/runtime/queued_items.rs:77-104`, `state/queue_migrations/0001_queued_items.sql:1-11`, `state/src/lib.rs:99-100`); impact requires an automation client to retry an ambiguously acknowledged call, and no such retry loop is present in-repo. |
| H003 | **Plausible** | Both queue-start paths await Core's `Started` result and only then delete the durable row; Core returns that result after `start_task` has spawned and registered the running task (`codex-rs/ext/queue/src/service.rs:391-400`, `codex-rs/ext/queue/src/service.rs:439-448`, `codex-rs/core/src/session/turn_input.rs:438-443`, `codex-rs/core/src/tasks/mod.rs:363-412`). A SQLite delete error propagates without cancelling the turn or removing the row, while task completion emits idle and the queue dispatches its head again, so one transient delete failure can cause one duplicate inference and persistent failures can repeat (`codex-rs/state/src/runtime/queued_items.rs:152-160`, `codex-rs/core/src/tasks/mod.rs:844-858`, `codex-rs/ext/queue/src/service.rs:405-448`, `codex-rs/ext/queue/src/service.rs:549-563`). Abrupt loss in the same window is also replayable after resume, but graceful shutdown is counterevidence because it records interruption and interrupted idle does not dispatch (`codex-rs/core/src/session/mod.rs:1341-1349`, `codex-rs/app-server/src/request_processors/thread_processor.rs:4010-4015`, `codex-rs/core/src/session/handlers.rs:398-404`, `codex-rs/ext/queue/src/service.rs:549-552`). |
| H004 | **Plausible** | `thread/start` accepts `ephemeral` and `threadSource` independently; omission resolves `ephemeral` to false, while the standard app-server source is `vscode` (`codex-rs/app-server-protocol/src/protocol/v2/thread.rs:108-118`, `codex-rs/core/src/config/mod.rs:4193`, `codex-rs/app-server/src/main.rs:38-45`). Durable threads persist both sources and set `memory_mode` only from `memories.generate_memories`; phase 1 allows `vscode` and filters on `threads.source`/`memory_mode`, not `thread_source`, so an idle automation rollout is claimable (`codex-rs/core/src/session/session.rs:818-855`, `codex-rs/rollout/src/lib.rs:74-80`, `codex-rs/memories/write/src/phase1.rs:159-175`, `codex-rs/state/src/runtime/memories.rs:218-244`). A later input turn starts the pipeline when a primary environment is configured, after which the gate checks only current-session ephemerality, `MemoryTool`, and non-root source (`codex-rs/app-server/src/request_processors/turn_processor.rs:673-684`, `codex-rs/memories/write/src/start.rs:33-37`). Each claim issues `stream_stage_one_prompt`; defaults permit two claims and then attempt phase 2, although `MemoryTool` is disabled by default (`codex-rs/memories/write/src/phase1.rs:204-218,315-317`, `codex-rs/memories/write/src/start.rs:78-81`, `codex-rs/config/src/types.rs:47-50`, `codex-rs/features/src/lib.rs:1035-1040`). |
| H005 | **Plausible** | Both phase-2 memory consolidation and review clone the parent `Config`, and neither disables `CodexHooks`; new sessions rebuild hooks from that cloned layer stack (`codex-rs/memories/write/src/phase2.rs:318-338`, `codex-rs/core/src/tasks/review.rs:105-136`, `codex-rs/core/src/session/mod.rs:4379-4384`). `UserPromptSubmit` and pre/post-tool dispatch lack Review/Memory gating, and only `ThreadSpawn` gets subagent metadata, so eligible hooks run as ordinary hooks (`codex-rs/core/src/hook_runtime.rs:182-213,280-304,600-625,948-957`). Their output is stored as developer context and included from history in model requests; the default per-fragment cap is 2,500 tokens, and an async result can force a second request (`codex-rs/core/src/hook_runtime.rs:764-783`, `codex-rs/core/src/session/turn.rs:371-375,411-425`, `codex-rs/hooks/src/output_spill.rs:12-23`, `codex-rs/core/tests/suite/hooks.rs:1608-1712`). Scope is narrower than the hypothesis wording: Review lifecycle hooks are gated, memory Stop filters user/project/plugin sources, and compact hooks execute but do not themselves return additional model context (`codex-rs/core/src/hook_runtime.rs:121-145,321-365,407-440`, `codex-rs/hooks/src/events/stop.rs:42-84`, `codex-rs/hooks/src/events/compact.rs:34-109`). |
| H006 | **Plausible** | Async `UserPromptSubmit` hooks are spawned into a session-owned task set with their originating turn ID, but rollback neither aborts nor filters that work; only session shutdown aborts hooks and drains the result queue (`codex-rs/hooks/src/engine/command_runner.rs:107-179`, `codex-rs/core/src/session/handlers.rs:258-327,398-406`). Once normal task completion clears `active_turn`, rollback is accepted and reconstruction removes the selected user turn (`codex-rs/core/src/tasks/mod.rs:842-857`, `codex-rs/core/src/session/rollout_reconstruction.rs:419-420`). The next turn drains every queued result without comparing `result.turn_id`, persists its context before the new prompt, and the integration test proves idle prior-turn hook context appears in the later model request (`codex-rs/core/src/session/turn.rs:162-163`, `codex-rs/core/src/hook_runtime.rs:702-716,764-775`, `codex-rs/core/tests/suite/hooks.rs:1718-1844`). |
| H007 | **Plausible** | Four or more enabled, trusted synchronous Stop handlers can each contribute a blocking reason; aggregation flattens every fragment and the 2,500-token spill limit is applied separately (`codex-rs/hooks/src/events/stop.rs:430-435`, `codex-rs/hooks/src/output_spill.rs:12,107-118`). The renderer creates one user message from all fragments without an aggregate admission check, records it, and immediately loops into the next request; ordinary messages remain unchanged in prompt history (`codex-rs/protocol/src/items.rs:623-643`, `codex-rs/core/src/session/turn.rs:521-537`, `codex-rs/core/src/context_manager/history.rs:207-224,525-537`). Stop groups and their handlers are uncapped vectors and discovery iterates every entry; integration tests confirm individual spilling plus multiple fragments in one model request (`codex-rs/config/src/hook_config.rs:58,147-151`, `codex-rs/hooks/src/engine/discovery.rs:464-502`, `codex-rs/core/tests/suite/hooks.rs:2375-2427,2484-2538`). |
| H008 | **Plausible** | Unified Exec appends process chunks unchanged and converts retained bytes into function-output text with lossy UTF-8 plus truncation only (`codex-rs/core/src/unified_exec/process.rs:502-507,546-550,607-612`; `codex-rs/core/src/tools/context.rs:372-379,454-457`; `codex-rs/utils/output-truncation/src/lib.rs:17-34`). The cited integration tests inspect `function_call_output` items in outbound model request bodies and find the `write_stdin` string repeated in output; another outbound-request assertion explicitly tolerates Windows control characters (`codex-rs/core/tests/suite/unified_exec.rs:160-170,2245-2248,2331-2360,3727-3736`). Duplication requires the PTY line discipline or child to echo/write the input and is bounded by collection/model caps; invalid UTF-8 becomes replacement text rather than remaining byte-exact. |
| H009 | **Plausible** | For a long-running command interrupted after registration but before the initial yield returns, the manager retains the live process while cancellation records only “aborted by user”; tests separately prove the PID survives and the next model request has no session ID (codex-rs/core/src/unified_exec/process_manager.rs:529-563; codex-rs/core/src/tools/parallel.rs:176-204,243-258; codex-rs/core/tests/suite/unified_exec.rs:2855-2938; codex-rs/core/tests/suite/abort_tasks.rs:213-300). The Begin event and live-terminal listing can expose the ID to clients, and the abort marker warns that processes may survive, but neither puts a usable ID in durable model history (codex-rs/core/src/unified_exec/process_manager.rs:529-536,1622-1638; codex-rs/core/src/context/turn_aborted.rs:10-11). Conversely, an ID-bearing “Process running” function output is persisted, shutdown kills/drains the manager, and cold resume replays ResponseItems into a freshly empty manager (codex-rs/core/src/tools/context.rs:359-378,488-499; codex-rs/core/src/session/mod.rs:3175-3220; codex-rs/core/src/session/handlers.rs:397-410; codex-rs/core/src/unified_exec/mod.rs:144-168; codex-rs/core/src/session/rollout_reconstruction.rs:378-428). If that output survives compaction/rollback, the advertised empty write poll reaches UnknownProcessId, becomes model-visible failure output, and triggers another sampling pass; rerunning the command is possible but model-dependent rather than guaranteed (codex-rs/core/src/tools/handlers/shell_spec.rs:113-150; codex-rs/core/src/unified_exec/process_manager.rs:775-780; codex-rs/core/src/tools/handlers/unified_exec/write_stdin.rs:84-113; codex-rs/core/src/stream_events_utils.rs:319-327; codex-rs/core/src/session/turn.rs:502-557). |
| H010 | **Plausible** | The model-facing collector starts one 50 ms post-exit close deadline, then returns even if output remains open; the subsequent state refresh immediately removes an exited process, preventing another poll (`codex-rs/core/src/unified_exec/process_manager.rs:1402-1466`, `codex-rs/core/src/unified_exec/process_manager.rs:1001-1018`). This window is realistic: the PTY tests explicitly allow a 200 ms Windows/ConPTY quiet drain after exit, and a driver test delivers valid tail output 50 ms after the exit signal (`codex-rs/utils/pty/src/tests.rs:110-133`, `codex-rs/utils/pty/src/tests.rs:792-836`). The driver adapter now waits for producer closure and the later streaming path may preserve the tail for UI completion, but the model response is already frozen from `raw_output` (`codex-rs/utils/pty/src/process.rs:412-422`, `codex-rs/core/src/unified_exec/async_watcher.rs:104-158`, `codex-rs/core/src/tools/context.rs:355-430`). Completion/delta events do not reconcile model history, so when the omitted tail is the sole failure detail or sentinel, obtaining it can require a full rerun (`codex-rs/rollout/src/policy.rs:90-170`, `codex-rs/core/src/session/rollout_reconstruction.rs:378-429`). |
| H011 | **Plausible** | With an attached environment and a model whose shell tools are not disabled, stable defaults expose both `exec_command` and `write_stdin` (`codex-rs/features/src/lib.rs:868-889`; `codex-rs/core/src/tools/spec_plan.rs:986-1012`; `codex-rs/core/tests/suite/tools.rs:908-916`). Their model-visible text says the command runs “in a PTY,” returns a session ID for ongoing interaction, and permits writing characters to an existing session; the `tty` field only says omission uses “plain pipes,” not that stdin is closed (`codex-rs/core/src/tools/handlers/shell_spec.rs:44-46,92-100,121-145`). Omission deserializes to false, a surviving non-TTY command can return a session ID, and every non-empty non-Ctrl-C write then deterministically returns `StdinClosed` (`codex-rs/core/src/tools/handlers/unified_exec.rs:34-35,68-69`; `codex-rs/core/tests/suite/unified_exec.rs:2018-2079,2447-2450,2532-2539`; `codex-rs/core/src/unified_exec/process_manager.rs:239,856-862,1283-1285`). The failed result explicitly tells the model to rerun with `tty=true` and forces follow-up sampling, although the rerun choice itself remains model-dependent (`codex-rs/core/src/unified_exec/errors.rs:19-22`; `codex-rs/core/src/tools/handlers/unified_exec/write_stdin.rs:104-112`; `codex-rs/core/src/tools/parallel.rs:73-86,216-239`; `codex-rs/core/src/stream_events_utils.rs:321-327`). |
| H012 | **Plausible** | Delta mode slices only the ordinary transcript; root authorization, retained trusted answers, and denied-read restrictions are gathered outside that branch and appended to every new prompt (`codex-rs/core/src/guardian/prompt.rs:142-178,228-265`). A completed malformed reply advances reusable-session state before parsing, while parse errors are retryable and the loop clones the same request, so the next delta turn repeats the evidence and action (`codex-rs/core/src/guardian/review_session.rs:1236-1247`; `codex-rs/core/src/guardian/review.rs:991-1004,1042-1075,1100-1115`; `codex-rs/core/src/guardian/prompt.rs:307-326`). Captured model requests retain the full first prompt and append the new delta message (`codex-rs/core/src/guardian/tests.rs:2581-2649`; `codex-rs/core/src/guardian/snapshots/codex_core__guardian__tests__guardian_followup_review_request_layout.snap:30-68`). Root and trusted-answer payloads are conditionally bounded near 8×900 tokens each, but denied-read entries have no local count/token cap; retries are capped at three and normal auto-compaction can eventually compact older history (`codex-rs/core/src/agent/control/user_authorization.rs:17-18,44-80`; `codex-rs/core/src/tools/handlers/request_user_input.rs:28-29,134-142`; `codex-rs/core/src/context/guardian_review_evidence.rs:16,73-107`; `codex-rs/core/src/guardian/prompt.rs:335-364`; `codex-rs/core/src/guardian/review.rs:78`; `codex-rs/core/src/session/turn.rs:171-176,1028-1051`). |
| H013 | **Plausible** | Direct model tool calls are recorded before their execution future is queued, and tool-start then snapshots that history, so the current call reaches the transcript (`codex-rs/core/src/stream_events_utils.rs:316-327`; `codex-rs/core/src/tools/lifecycle.rs:28-41`). With `ToolCalls` enabled, the transcript emits the call arguments while Guardian separately renders the same `ToolStartInput` payload as planned-action JSON in the same classifier message; the exact request assertion contains `{"path":"README.md"}` in both copies (`codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs:210-226`; `codex-rs/ext/guardian-v2/src/async_scorer/extension.rs:466-468,520-522,593-598`; `codex-rs/ext/guardian-v2/src/async_scorer/extension_tests.rs:1941-1959`). The duplicate transcript copy defaults to a 1,000-token entry cap, and the action/tool-entry/tool-transcript limits all accept 100,000, supporting roughly 100K overlap when jointly raised; disabling tool-call transcript input, eviction, or a pre-tool hook rewrite can eliminate or change it (`codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs:22-24,71-76,343-350`; `codex-rs/ext/guardian-v2/src/async_scorer/config.rs:167-200,308-315`; `codex-rs/features/src/tests.rs:267-284`; `codex-rs/core/src/tools/registry.rs:575-623`). |
| H014 | **Plausible** | All three implementations forward the effective summary setting into the shared Responses request builder, which emits `reasoning.summary` only when the model supports it and the setting is not `none` (`codex-rs/core/src/compact.rs:742-751`; `codex-rs/core/src/compact_remote_request.rs:82-89`; `codex-rs/core/src/compact_remote_v2.rs:378-395`; `codex-rs/core/src/client.rs:866-876`). SSE normalization surfaces summary deltas/done and completed `ResponseItem::Reasoning` items, but local replacement selects only assistant text, v1 filters reasoning, and v2 keeps only the compaction item while retaining provider usage (`codex-rs/codex-api/src/sse/responses.rs:403-478`; `codex-rs/core/src/compact.rs:352-359,763-793`; `codex-rs/core/src/compact_remote.rs:344-385`; `codex-rs/core/src/compact_remote_v2.rs:417-476`). Impact is conditional rather than universal: unsupported or disabled summaries are omitted, most bundled models currently default to `none`, and tests prove request propagation/ignored extra output but not that every provider emits or separately bills a summary, especially unary v1 whose decoded response exposes no usage (`codex-rs/core/tests/suite/client.rs:2446-2494,2651-2734`; `codex-rs/models-manager/models.json:34-1096`; `codex-rs/core/tests/suite/compact.rs:2287-2369`; `codex-rs/codex-api/src/endpoint/compact.rs:63-88`). |
| H015 | **Plausible** | On the first non-empty `OutputTextDelta`, only that delta is checked against 8 KiB; the receiver and lease move to a background task that ignores later deltas until `Completed`, while the caller returns immediately (`codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs:653-685`). A test sends a contradictory later delta and still receives the first classification (`codex-rs/ext/guardian-v2/src/async_scorer/sampler_tests.rs:633-651`). The WebSocket request shape has no `max_output_tokens`, and ordinary `/responses` is the default because `free_guardian` defaults false, so later output remains metered unless the eligible unmetered Guardian endpoint is enabled (`codex-rs/codex-api/src/common.rs:302-389`; `codex-rs/core/src/config/mod.rs:1493-1501`; `codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs:234-253`). At most 16 generations can occupy the pool, and a 17th request preferentially supersedes a scored drain, making the concurrency impact bounded and conditional (`codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs:55-57,204-209,542-549,666-670`). |
| H016 | **Plausible** | Ordinary retained messages are charged only text/content tokens with a one-token floor, while the existing full-item estimator charges serialized item bytes (`codex-rs/core/src/compact_remote_v2.rs:601-619,689-702`; `codex-rs/core/src/context_manager/history.rs:696-703,727-773`). Retained user messages stay as distinct items through prompt normalization, receive IDs and passthrough metadata, and remain in the Responses request; HTTP serializes that request whole, while WebSocket deltas require a strict input extension that a replacement history cannot satisfy (`codex-rs/core/src/context_manager/history.rs:210-223,463-477`; `codex-rs/core/src/session/mod.rs:3059-3187`; `codex-rs/core/src/client.rs:903-990,1817-1860`; `codex-rs/codex-api/src/endpoint/responses.rs:102-116`). The one-token floor bounds retained sources at 64K but still admits 13.6K tiny messages, and compaction returns directly to sampling without a second context admission check (`codex-rs/core/src/compact_remote_v2.rs:74-78,480-509`; `codex-rs/core/src/session/turn.rs:371-390,1028-1054`). Thus the framing omission is reachable and can be material; the exact 13.6K/272K crossing remains provider-tokenizer dependent because not every wire field is necessarily model-accounted. |
| H017 | **Plausible** | Local compaction appends and persists a user-role `CompactionSummary` with `compaction.summary` metadata but empty text markers; contextual recognition consults only text matchers and omits that type, so `parse_turn_item` classifies it as a real `UserMessage` (`codex-rs/core/src/compact.rs:356-379,722-730`; `codex-rs/core/src/context/compaction_summary.rs:17-35`; `codex-rs/core/src/context/contextual_user_message.rs:18-43`; `codex-rs/core/src/event_mapping.rs:64-65,98-100,189-191`). Remote v2 retains user messages accepted by the shared predicate, truncates them into the 64K retained budget, and then appends the new compaction item, so a surviving prior summary remains alongside v2 output (`codex-rs/core/src/compact_remote.rs:370-377`; `codex-rs/core/src/compact_remote_v2.rs:493-509,560-564,585-670`). This is reachable after resume because modern replacement histories replay verbatim and legacy checkpoints are rebuilt with `CompactionSummary`; tests cover legacy reconstruction and immediate v2 compact-after-resume, while no marker or content-kind filter closes the gap (`codex-rs/core/src/session/rollout_reconstruction.rs:370-416`; `codex-rs/core/src/session/rollout_reconstruction_tests.rs:1255-1283`; `codex-rs/core/tests/suite/compact_remote.rs:533-591`). |
| H018 | **Plausible** | A new session starts with empty model history and defers initial context until the first real turn, but manual remote-v2 unconditionally adds `compaction_trigger` and sends the normal model stream with base instructions and all model-visible tools (`codex-rs/core/src/state/session.rs:66-70`, `codex-rs/core/src/session/mod.rs:1330-1336`, `codex-rs/core/src/compact_remote_v2_attempt.rs:41-108`). The pristine remote-v2 budget test immediately submits `Op::Compact` and observes one usage-bearing request; on any successful exactly-one-compaction response, the returned item is unconditionally appended, installed, and persisted as replacement history (`codex-rs/core/tests/suite/rollout_budget.rs:339-404`, `codex-rs/core/src/compact_remote_v2.rs:300-328,480-509`, `codex-rs/core/src/session/mod.rs:3496-3534`). This path requires TokenBudget off, v2 provider support, the stable v2 feature, continuing hooks, and an upstream compaction item; a zero-item server no-op instead becomes a fatal error and persists nothing, so live production billing is not proven (`codex-rs/core/src/tasks/compact.rs:34-50`, `codex-rs/core/src/compact_remote_v2.rs:450-463`). V1 explicitly short-circuits empty input and tests zero remote requests, while both `/compact` and `thread/compact/start` expose no pristine-history guard (`codex-rs/core/src/client.rs:579-580`, `codex-rs/core/tests/suite/compact_remote.rs:4884-4901`, `codex-rs/tui/src/chatwidget/slash_dispatch.rs:263-276`, `codex-rs/app-server/src/request_processors/thread_processor.rs:2344-2356`). |
| H019 | **Unlikely** | The legacy SSE loop does have the counterfactual overwrite: it stores most `response.failed` errors, but a later framing error or idle timeout emits a fresh `ApiError::Stream`; only clean EOF reuses the stored error (`codex-api/src/sse/responses.rs:702-729,810-819`). That replacement would map to retryable `CodexErr::Stream` and reach the sampling retry loop, whose HTTP budget is five retries by default and capped at 100 (`codex-api/src/api_bridge.rs:26-40`; `protocol/src/error.rs:380-415`; `core/src/session/turn.rs:1380-1459`; `model-provider-info/src/lib.rs:29-35,369-372`). However, production Responses traffic uses WebSocket when supported and otherwise returns `UnsupportedOperation`; SSE is behind a false-by-default test-only switch (`core/src/client.rs:176-183,2024-2063`; `core/src/client_tests.rs:137-173`). The active WebSocket loop returns semantic errors immediately, with a passing integration test covering no close handshake (`codex-api/src/endpoint/responses_websocket.rs:832-843`; `core/tests/suite/client_websockets.rs:2461-2518`). |
| H020 | **Plausible** | A cold root resume loads persisted history and then constructs a fresh `AgentControl`; its new `RolloutBudget` initializes `weighted_tokens_used` to zero, while reconstruction restores conversation state and only the last persisted token-usage snapshot (`codex-rs/core/src/thread_manager.rs:1023-1039`, `codex-rs/core/src/thread_manager.rs:1076-1110`, `codex-rs/core/src/agent/control.rs:145-160`, `codex-rs/core/src/rollout_budget.rs:35-64`, `codex-rs/core/src/session/mod.rs:1330-1379`). Provider budget units are explicitly omitted from serialization and the budget has no history-seeding path, so stopped/unloaded root resumes reopen enforcement; an already-running resume instead returns the existing thread/control (`codex-rs/protocol/src/protocol.rs:2186-2190`, `codex-rs/core/src/thread_manager.rs:1873-1893`, `codex-rs/core/src/thread_manager_tests.rs:1766-1892`). The feature is under-development and disabled by default, so this is opt-in rather than default-reachable; once configured, uncapped repeated cold resumes can each accrue a fresh allowance (plus threshold-crossing response overshoot), making the local cap effectively unbounded (`codex-rs/features/src/lib.rs:1495-1500`, `codex-rs/core/src/config/mod.rs:2785-2806`). |
| H021 | **Plausible** | Resume replays every persisted `ResponseItem`, but a fresh `SessionState` initializes the time state and token/fallback gates unclaimed, while reconstruction restores only history/world state and the window number/IDs (`codex-rs/core/src/session/rollout_reconstruction.rs:378-384`, `codex-rs/core/src/state/session.rs:66-79`, `codex-rs/core/src/session/mod.rs:1506-1523`, `codex-rs/core/src/state/auto_compact_window.rs:72-74`). Thus the next request in the same restored window sees `last_window_id = None` and immediately appends another time reminder, while the post-sampling `<= threshold`/zero-remaining paths can claim and append the token reminder and fallback again (`codex-rs/core/src/session/time_reminder.rs:68-90`, `codex-rs/core/src/session/turn.rs:360-469`, `codex-rs/core/src/session/token_budget.rs:88-125`). This is reachable for Persistent-effort current-time defaults absent an explicit override and for enabled token-budget configurations; at zero remaining the built-in pair is about 426 raw-text heuristic tokens, while nonzero threshold crossings add only about 229-230, and two configured 2 KB bodies cap the raw-text estimate near 1000 plus about 20 for time (`codex-rs/core/src/session/turn_context.rs:731-743`, `codex-rs/models-manager/models.json:79-84`, `codex-rs/core/src/config/mod.rs:1111-1165`). |
| H022 | **Plausible** | V2 completion mail is created with `trigger_turn = false`, and the handler starts idle work only for trigger mail or an outstanding durable sleep, so an ordinary idle parent leaves the result queued (`codex-rs/core/src/session/mod.rs:2119-2149`; `codex-rs/core/src/session/handlers.rs:81-95`). A fresh user turn carries only its explicit input into `run_turn`; nonempty input disables the initial pending-input drain, and when the first sample has not established a final-answer boundary, the post-sample mailbox check forces the next loop/request (`codex-rs/core/src/session/turn_input.rs:413-420`; `codex-rs/core/src/session/turn.rs:267,307-315,413-425`). The V2 integration test requires a first request without the completion and a second request containing it (`codex-rs/core/tests/suite/subagent_notifications.rs:2585-2628`), although final-answer text defers the mail to a later turn rather than causing a same-turn second call (`codex-rs/core/src/stream_events_utils.rs:486-499`; `codex-rs/core/tests/suite/pending_input.rs:978-1038`). |
| H023 | **Plausible** | The detached watcher stops at the first final status, injects one notification, and then its task ends; its only call sites are initial spawn and rollout resume (`codex-rs/core/src/agent/control.rs:583-658`, `codex-rs/core/src/agent/control/spawn.rs:768-779,1231-1246`). `send_input` on a still-live completed agent only starts or steers another turn and returns a submission ID, while the per-turn terminal forwarding path rejects V1 (`codex-rs/core/src/agent/control.rs:184-210`, `codex-rs/core/src/tools/handlers/multi_agents/send_input.rs:89-130,149-151`, `codex-rs/core/src/session/mod.rs:2009-2016,2256-2262`). Therefore live V1 reuse, which the tool spec explicitly encourages, cannot automatically expose the second result; obtaining it through `wait_agent` requires a tool call and thus a subsequent parent sampling request (`codex-rs/core/src/tools/handlers/multi_agents_spec.rs:176,269-280`, `codex-rs/core/src/stream_events_utils.rs:297-327`, `codex-rs/core/src/session/turn.rs:150-153`). A close-and-resume falls outside this condition because rollout resume attaches a fresh watcher (`codex-rs/core/src/agent/control/spawn.rs:1231-1246`). |
| H024 | **Plausible** | `bounded_completion_status` truncates the raw completion at 900 heuristic tokens, then V1 serializes it inside JSON and XML without a post-render cap: a 100,000-byte quote payload renders to 7,326 bytes (~1,832 heuristic tokens), while NUL renders to 21,726 bytes (~5,432) (`codex-rs/core/src/session_prefix.rs:8-25`, `codex-rs/core/src/context/subagent_notification.rs:34-45`, `codex-rs/context-fragments/src/fragment.rs:83-91`). History leaves ordinary `Message` and `AgentMessage` items unchanged and request construction forwards them unchanged; aggregate accounting can compact only at the global context threshold, not restore the 1,000-token item cap (`codex-rs/core/src/context_manager/history.rs:479-536`, `codex-rs/core/src/session/turn.rs:1028-1055`, `codex-rs/core/src/client.rs:885-932`). V2 `task_name` has no schema or validation length cap and parent/child paths are duplicated in completion text plus structured author/recipient fields, so a reachable 10,000-byte segment contributes 20,022 path bytes (`codex-rs/core/src/tools/handlers/multi_agents_spec.rs:120-140`, `codex-rs/protocol/src/agent_path.rs:111-151`, `codex-rs/core/src/context/inter_agent_completion_message.rs:39-44`, `codex-rs/protocol/src/protocol.rs:783-890`). |
| H025 | **Plausible** | `ThreadRollbackFailed` is explicitly nonterminal for turn status, but `agent_status_from_event` converts every `EventMsg::Error` to final `AgentStatus::Errored`, and delivery publishes it on the status watch (`codex-rs/protocol/src/protocol.rs:1856-1877,2043-2049`; `codex-rs/core/src/agent/status.rs:6-30`; `codex-rs/core/src/session/mod.rs:2256-2262`). A loaded, non-paginated V1 child accepts direct input, so `thread/rollback` during its active turn reaches Core and emits this error without stopping the turn (`codex-rs/app-server/src/request_processors/thread_input.rs:8-20`; `codex-rs/app-server/src/request_processors/thread_processor.rs:2283-2336`; `codex-rs/core/src/session/handlers.rs:245-269`). The one-shot V1 watcher then injects the false terminal status and exits; the eventual `TurnComplete` overwrites the watch value, but only spawn/resume attaches a watcher, so genuine completion is not automatically delivered (`codex-rs/core/src/agent/control.rs:565-658`; `codex-rs/core/src/tasks/mod.rs:817-839`; `codex-rs/core/src/agent/control/spawn.rs:768-780,1235-1247`). |
| H026 | **Plausible** | In effective hybrid `CodeMode`, an eligible `Direct` function remains natively visible while its description is rebuilt from that same `parameters` schema as a TypeScript declaration; request tests confirm the augmented tool survives both Responses and Responses Lite construction (`core/src/tools/spec_plan.rs:496-551`; `tools/src/code_mode.rs:8-43,124-160`; `core/tests/suite/code_mode.rs:5259-5459`). The client and raw request serializers preserve both fields, so each such input shape has two model-visible representations (`core/src/client.rs:896-932`; `codex-api/src/common.rs:293-340`; `codex-api/src/endpoint/responses.rs:102-117`). Scope is narrower than claimed: `output_schema` is not serialized natively, and CodeModeOnly, deferred, excluded, DirectModelOnly, collision-shadowed, freeform, or provider-omitted namespace tools do not have duplicate schemas (`tools/src/responses_api.rs:31-43`; `core/src/tools/spec_plan.rs:694-809`). |
| H027 | **Plausible** | Local compaction replaces history with retained user messages plus a generated summary without touching Code Mode runtime (`codex-rs/core/src/compact.rs:352-390`). Its Code Mode test explicitly proves `call-1` is absent after compaction while the still-live cell is terminable only because the mock injects the saved ID out of band (`codex-rs/core/tests/suite/code_mode.rs:1275-1305`). On cold resume, persisted response items are replayed into model history but each `Session` creates a fresh service backed by empty per-session cells/store, so an old handle fails as “not found” and stored values are absent (`codex-rs/core/src/session/rollout_reconstruction.rs:375-384`; `codex-rs/core/src/session/session.rs:1433-1437`; `codex-rs/code-mode-runtime/src/session_runtime/mod.rs:44-68`; `codex-rs/code-mode-runtime/src/service.rs:389-398`). Reconnect similarly opens a new generation and rejects old IDs—good anti-aliasing cleanup, but no model-history invalidation—so the defect is gated on Code Mode and a live cell or relied-on store value crossing one of these boundaries (`codex-rs/code-mode/src/grpc_session/reconnect.rs:121-167`; `codex-rs/code-mode/src/grpc_session/generation.rs:57-67`). |
| H028 | **Plausible** | The session-wide broker retains only a cell gate/item ID, while every sampling request starts a worker backed by that request's `StepContext`; queued `InvokeTool` messages are consumed by whichever request worker receives them and routed through its router (`codex-rs/core/src/tools/code_mode/delegate.rs:27-42,101-183,257-283`; `codex-rs/core/src/session/turn.rs:1370-1379`; `codex-rs/core/src/tools/parallel.rs:41-45,112-165`). A yielded cell is released after its originating turn and its delayed nested `exec_command` completes through a later-turn flow (`codex-rs/core/tests/suite/code_mode.rs:3158-3165,3228-3277`); step-local handlers also demonstrably change generation, so catalog removal/change can reach the registry's unsupported-call failure (`codex-rs/core/tests/suite/code_mode.rs:4926-4935,5063-5103`; `codex-rs/core/src/tools/registry.rs:518-522,819-823`). `CodeModeInterrupt` enumerates every live broker gate, not cells owned by the interrupted turn, and the test expects both an older background cell and the current cell to be gone afterward (`codex-rs/core/src/tasks/mod.rs:909-919`; `codex-rs/core/src/tools/code_mode/mod.rs:146-160`; `codex-rs/core/tests/suite/code_mode.rs:3538-3569,3620-3623`). Literal mid-session feature disablement is counterevidence because features are session-invariant, but a later effective Direct-mode request still creates no dispatch worker (`codex-rs/core/src/session/session.rs:51-53`; `codex-rs/core/src/tools/code_mode/mod.rs:200-219`). |
| H029 | **Plausible** | The WebSocket mapper copies each `OutputItemDone` into `LastResponse` before turn handling mutates its separate copy (`codex-rs/core/src/client.rs:2190-2194`, `codex-rs/core/src/client.rs:2221-2225`). Missing or empty IDs are then synthesized before that turn copy is recorded (`codex-rs/core/src/session/turn.rs:2181-2193`, `codex-rs/core/src/session/turn.rs:2324-2325`, `codex-rs/core/src/session/mod.rs:3107-3128`), while continuation matching uses full `ResponseItem` equality and falls back to a full request without `previous_response_id` on mismatch (`codex-rs/core/src/client.rs:387-399`, `codex-rs/core/src/client.rs:1315-1348`, `codex-rs/core/src/client.rs:1374-1379`, `codex-rs/core/src/client.rs:1833-1854`). The precondition is reachable for function calls because their wire ID is optional and the event parser accepts the item (`codex-rs/protocol/src/models.rs:1037-1041`, `codex-rs/codex-api/src/sse/responses.rs:403-418`); conforming providers that always emit non-empty IDs avoid the bug. |
| H030 | **Plausible** | `serviceTierForTurn` can make the first request differ from the already-issued startup prewarm, and `service_tier` is an explicit equality term in the incremental-continuation gate (`codex-rs/app-server-protocol/src/protocol/v2/turn.rs:222-226`; `codex-rs/core/src/session_startup_prewarm.rs:273-329`; `codex-rs/core/src/client.rs:330-384`). A mismatch returns no parent/delta, so the caller sends the full `request.input` built from cloned conversation history; tests observe no `previous_response_id` for both prewarm-to-first-turn and later tier transitions (`codex-rs/core/src/session/turn.rs:371-390`; `codex-rs/core/src/client.rs:1315-1389,1833-1854`; `codex-rs/core/tests/suite/agent_websocket.rs:372-420,483-552`). OpenAI documents `service_tier` as a per-request processing choice and WebSocket `previous_response_id` as lineage, with no tier-isolation rule, so the replay appears avoidable when the resolved tier actually changes and all other request properties/history prefixes remain compatible. The prewarm is only partly orphaned: it cannot donate its response ID after the mismatch, but the warmed socket is still reused (`codex-rs/core/tests/suite/agent_websocket.rs:393-406`). |
| H031 | **Plausible** | Cached model-visible tools explicitly skip startup waiting and are added to a binding without a callable client; the end-to-end test observes their old descriptions in the Responses request while the server is `NotStarted` (`codex-mcp/src/connection_manager/tool_catalog.rs:179-246,288-296`; `core/tests/suite/mcp_tool_cache.rs:519-565,646-657`). For regular MCP, this is bounded to an identity-scoped process cache with a 30-minute TTL and is repeatedly costly chiefly for dormant cached subagents when tools are directly exposed rather than deferred behind search (`codex-mcp/src/tool_catalog_cache.rs:32-38,132-173`; `core/src/session/mcp_runtime.rs:361-366`; `core/src/mcp_tool_exposure.rs:84-94`). Host-owned Apps cache is account/workspace scoped, bypassed for env-bearer auth, and eagerly refreshes, but its <=32 MiB disk snapshot has only schema/size checks and no age rejection, so it can be stale during the startup window (`codex-mcp/src/connection_manager.rs:329-344,507-556`; `connectors/src/connector_runtime/persistence.rs:35,57-68,138-164`). Calls rebind to the live catalog rather than execute against cache, yet a stale tool removed from live model visibility yields a model-visible failure on the continuation, which the integration test demonstrates (`core/src/session/mcp_runtime.rs:60-71`; `core/src/mcp_tool_call.rs:155-172`; `core/tests/suite/mcp_tool_cache.rs:567-688`). |
| H032 | **Plausible** | For ready regular servers, `tools/list_changed` only logs; same-identity reconciliation reuses the frozen client, and a test confirms a new binding still exposes `old_search` after the live server switches to `new_search` (`codex-rs/rmcp-client/src/logging_client_handler.rs:82-88`; `codex-rs/codex-mcp/src/rmcp_client.rs:112-124`; `codex-rs/codex-mcp/src/connection_manager_tests.rs:4497-4623`). HTTP 404 recovery replaces only the rmcp service, while the upper tools/instructions/server-info/capability fields remain the startup snapshot used to rebuild model tools (`codex-rs/rmcp-client/src/rmcp_client.rs:1428-1483`; `codex-rs/codex-mcp/src/rmcp_client.rs:926-969`; `codex-rs/codex-mcp/src/connection_manager/tool_catalog.rs:248-260`). Because the regular catalog revision is not advanced, the stale call is sent by raw name and failures become model tool output followed by another sampling request (`codex-rs/codex-mcp/src/binding.rs:293-361`; `codex-rs/core/src/mcp_tool_call.rs:500-564`; `codex-rs/core/src/stream_events_utils.rs:319-327`). Codex Apps hard-refresh and explicit, identity-change, or closed-transport reconnects are real adoption paths, so the claim applies to unforced same-identity regular-server changes and recovered model-visible metadata, not all metadata forever (`codex-rs/codex-mcp/src/connection_manager/tool_catalog.rs:351-405`; `codex-rs/core/src/session/handlers.rs:227-229`; `codex-rs/codex-mcp/src/runtime.rs:247-312`; `codex-rs/codex-mcp/src/connection_manager_tests.rs:5074-5170`). |
| H033 | **Plausible** | Both preparation paths append the same plugin-display-name sentence to every attributed tool, and conversion plus namespace coalescing preserve each child description without deduplicating that prose (`codex-rs/codex-mcp/src/rmcp_client.rs:672-733`, `codex-rs/tools/src/mcp_tool.rs:36-54`, `codex-rs/core/src/tools/spec_plan.rs:828-877`). For a six-byte display name such as `sample`, each copy adds 37–39 bytes, which is about 925–1,000 tokens across 100 children under the repository's four-bytes-per-token estimate (`codex-rs/utils/string/src/truncate.rs:71-83`). The 100-tool per-request impact is conditional: search-capable namespace models defer MCP tools and return at most 32 leaves, while Agent Plugin description truncation can remove an appended tail (`codex-rs/core/src/mcp_tool_exposure.rs:90-93`, `codex-rs/tools/src/tool_discovery.rs:8-12`, `codex-rs/tools/src/mcp_tool.rs:7-16`). |
| H034 | **Plausible** | `capture_binding_with_metadata` normalizes every config-allowed tool and resolves collisions before checking `_meta.ui.visibility`, so a distinct hidden tool whose raw name sanitizes to the same namespace/name can add the 13-byte `_` plus 12-hex suffix to a visible tool and then be dropped (`codex-rs/codex-mcp/src/connection_manager/tool_catalog.rs:259-315`; `codex-rs/codex-mcp/src/tools.rs:120-197,227-249`). That stored callable name becomes the MCP namespace/child spec and is serialized on every request (or hashed into the Responses Lite `AdditionalTools` item ID); a changed catalog also fails WebSocket incremental-request property matching (`codex-rs/core/src/tools/handlers/mcp.rs:452-483`; `codex-rs/core/src/client.rs:330-378,894-932,1312-1324`). Hashing and ordering are deterministic for an unchanged full catalog, but a refresh that adds or removes the hidden collider toggles base versus suffixed names; a previously recorded call keeps its old name in replayed history (`codex-rs/codex-mcp/src/tools.rs:197-205,243-249`; `codex-rs/core/src/stream_events_utils.rs:299-317`; `codex-rs/core/src/context_manager/history.rs:207-224,525-538`). |
| H035 | **Plausible** | Overflow removal mutates only a copy-on-write history clone, but success re-clones live session history and rebuilds from it; any real user item trimmed deeply enough to make the retry fit is therefore reinstalled (`codex-rs/core/src/compact.rs:257-320,352-390`; `codex-rs/core/src/context_manager/history.rs:48-52,285-295`). The 20K selector debits only concatenated text (media maps to an empty string), while per-item JSON/IDs, the summary, and context injected after selection are outside that budget (`codex-rs/protocol/src/items.rs:483-500`; `codex-rs/core/src/compact.rs:540-569,644-730,364-372`). Inline pre-/mid-turn compaction then proceeds to sampling without another token admission; prompt normalization preserves empty `InputText` messages, and the normal request path returns `ContextWindowExceeded` rather than trimming (`codex-rs/core/src/session/turn.rs:350-390,471-499,1028-1055,1390-1425`; `codex-rs/core/src/context_manager/history.rs:459-477`). The 1,000-empty-message case is reachable through repeated media-turn compactions; the 20,000 one-byte ceiling needs a sufficiently large/resumed history or model downshift, while standalone manual compaction has no immediate normal follow-up (`codex-rs/core/src/session/turn.rs:1094-1185`; `codex-rs/core/src/tasks/compact.rs:28-84`). |
| H036 | **Plausible** | `/compact` and v2 `thread/compact/start` submit `Op::Compact` without inspecting history, and the core handler starts `CompactTask` unconditionally (`codex-rs/tui/src/chatwidget/slash_dispatch.rs:263-273`, `codex-rs/app-server/src/request_processors/thread_processor.rs:2347-2356`, `codex-rs/core/src/session/handlers.rs:236-242`). With token-budget disabled (the default) and `RemoteCompactionSupport::Unsupported`, local compaction appends the summarization prompt, performs a real streamed model request, records its output, and installs a summary; the empty-history test proves both the request and retained `MANUAL_EMPTY_SUMMARY` (`codex-rs/features/src/lib.rs:1489-1494`, `codex-rs/core/src/tasks/compact.rs:34-76`, `codex-rs/core/src/compact.rs:245-294`, `codex-rs/core/tests/suite/compact.rs:5068-5123`). An immediate repeat is also unguarded: the prior summary enters the next request, is filtered only while rebuilding preserved user messages, and is replaced by the newly generated summary (`codex-rs/core/src/compact.rs:245-285`, `codex-rs/core/src/compact.rs:352-388`, `codex-rs/core/src/compact.rs:540-573`). A stopping pre-compact hook or enabled token-budget mode avoids the local model call; separately, remote compaction guards truly empty input (`codex-rs/core/src/compact.rs:194-209`, `codex-rs/core/src/client.rs:579-581`). |
| H037 | **Plausible** | With an effective `auto`, `concise`, or `detailed` summary on a model that supports the parameter, local, remote v1, and remote v2 compaction all forward `reasoning.summary`; a reachable local-compaction test observes `detailed` on the compact request (`core/src/compact.rs:742-750`, `core/src/compact_remote_request.rs:82-95`, `core/src/compact_remote_v2.rs:382-391`, `core/tests/suite/compact.rs:2305-2374`). Local compaction ignores reasoning-summary stream events and rebuilds installed history from user messages plus assistant text, remote v1 rejects `ResponseItem::Reasoning`, and v2 retains only the `Compaction` item while preserving completed token usage (`core/src/compact.rs:352-359`, `core/src/compact.rs:735-795`, `core/src/compact_remote.rs:370-397`, `core/src/compact_remote_v2.rs:302-309`, `core/src/compact_remote_v2.rs:417-477`). Provider-generated plaintext summaries are therefore billed but absent from installed history and consumer item/delta output; H037 is an aggregation duplicate of H014. |
| H038 | **Plausible** | When the selected model supports search and the provider supports namespace tools, ordinary policy-eligible MCP tools are assigned deferred with no catalog-size or explicit-selection branch; two-tool and explicit `app://calendar` tests lock in that behavior (`codex-rs/core/src/tools/spec_plan.rs:590-602`, `codex-rs/core/src/mcp_tool_exposure.rs:84-98`, `codex-rs/core/src/mcp_tool_exposure_test.rs:507-518`, `codex-rs/core/tests/suite/search_tool.rs:209-242,509-551`). Client search exposes matches only for the next model call, and the exercised search→MCP call→final flow takes three sampling requests; direct exposure remains a valid local policy outcome, so the added inference is avoidable rather than provider-mandated (`codex-rs/core/src/tools/handlers/tool_search_spec.rs:120-126`, `codex-rs/core/tests/suite/search_tool.rs:558-676`, `codex-rs/core/src/tools/spec_plan.rs:231-255`). Always-defer is nevertheless intentional Codex policy, and without latency/token measurements its net cost is uncertain because it trades an extra sampling call for smaller repeated tool schemas/cache pressure (`codex-rs/features/src/lib.rs:203-206`). |
| H039 | **Plausible** | Ordinary local compaction sends `history.for_prompt(...)`, and v1/v2 clone the same history before applying only the remote overflow trimmer; paired `ToolSearchCall`/`ToolSearchOutput` items therefore reach the request (`codex-rs/core/src/compact.rs:266-283`, `codex-rs/core/src/compact_remote_request.rs:33-65`, `codex-rs/core/src/compact_remote_v2_attempt.rs:41-86`). Each output contains serialized tool definitions, including parameter schemas, bounded at 32 KiB (about 8.2K tokens), while local replacement rebuilds only user messages plus summary and both remote installers reject tool-search items (`codex-rs/core/src/tools/context.rs:178-216`, `codex-rs/tools/src/tool_discovery.rs:7-17`, `codex-rs/core/src/compact.rs:352-383,644-732`, `codex-rs/core/src/compact_remote.rs:344-395`, `codex-rs/core/src/compact_remote_v2.rs:480-500,534-565`). The only existing schema elision empties a trailing search output after the estimated request already exceeds the context window, as the integration test confirms; below-window attempts and eligible retries/model fallbacks resend it (`codex-rs/core/src/compact_remote.rs:399-505`, `codex-rs/core/tests/suite/compact_remote.rs:2703-2800`, `codex-rs/core/src/compact_remote_v2.rs:368-415`, `codex-rs/core/src/compact_model_fallback.rs:7-19`). A latest unconsumed search result can matter during mid-turn continuation, so a fix should preserve minimal/live discovery state rather than blindly strip every pair (`codex-rs/core/src/session/turn.rs:460-500`, `codex-rs/core/src/tools/spec_plan.rs:496-521`). |
| H040 | **Plausible** | The handler preserves `SearchEngine::search(query, limit)` order into coalescing, which appends later children of a repeated namespace behind its first occurrence (`codex-rs/core/src/tools/handlers/tool_search.rs:237-258`; `codex-rs/tools/src/responses_api.rs:90-117`). The byte limiter breaks at the first non-fitting namespace child rather than skipping it, so later children in that coalesced namespace vanish; the response is nevertheless hard-coded `completed` and carries no partial marker (`codex-rs/tools/src/tool_discovery.rs:48-70`; `codex-rs/core/src/tools/context.rs:192-198`). This is conditional to later matches sharing that namespace (outer namespaces continue), but search has only `query`/`limit`, and follow-ups rely on search-output history rather than tool reinjection, so an unknown omitted tool requires another search that can preserve the same ranking (`codex-rs/protocol/src/models.rs:2029-2034`; `codex-rs/core/tests/suite/search_tool.rs:802-814`). |
| H041 | **Plausible** | Client `ToolSearchCall`s are persisted before their futures are queued, while matching outputs are recorded only after `drain_in_flight` resolves, so a hard crash in that window leaves a lone call (`codex-rs/core/src/stream_events_utils.rs:303-327`; `codex-rs/core/src/session/turn.rs:2155-2165`). Cold reconstruction replays that item verbatim, and prompt normalization inserts `ToolSearchOutput { status: "completed", execution: "client", tools: [] }` (`codex-rs/core/src/session/rollout_reconstruction.rs:378-384`; `codex-rs/core/src/context_manager/normalize.rs:69-85`; `codex-rs/core/src/context_manager/history_tests.rs:2031-2065`). `for_prompt` returns the normalized pair without filtering, so recovery/continuation of a task requiring a deferred tool must search again to obtain its schema and that tool call forces another follow-up inference; an unrelated next user request need not (`codex-rs/core/src/context_manager/history.rs:210-224,463-470`; `codex-rs/core/tests/suite/search_tool.rs:763-813`; `codex-rs/core/src/session/turn.rs:399-425,502-563`). |
| H042 | **Plausible** | Phase 1 loads every append-only rollout item, retains `ToolSearchCall`/`ToolSearchOutput`, and discards `Compacted` itself, so it never applies checkpoint `replacement_history` (`codex-rs/rollout/src/recorder.rs:1026-1076`, `codex-rs/rollout/src/policy.rs:64-78`, `codex-rs/memories/write/src/phase1.rs:290-291,404-428`). By contrast, compaction replaces live history and resume begins from the newest replacement checkpoint plus its surviving suffix, making older schemas inactive (`codex-rs/core/src/session/mod.rs:3496-3534`, `codex-rs/core/src/session/rollout_reconstruction.rs:180-222,338-378`). Each discovery result is bounded to 32 KiB/32 leaves and the upload is head/tail-capped at 70% of effective input, but there is no aggregate historical-schema filter; impact requires the default-off `memories` feature and enough searches (`codex-rs/tools/src/tool_discovery.rs:9-17,34-68`, `codex-rs/memories/write/src/prompts.rs:98-118`, `codex-rs/features/src/lib.rs:1035-1040`). |
| H043 | **Plausible** | The external `thread/realtime/start.prompt` is copied into core unchanged, while validation caps only request start/end instructions and initial items; that prompt and configured backend/startup overrides are combined into `RealtimeSessionConfig.instructions` without an aggregate bound (`codex-rs/app-server/src/request_processors/turn_processor.rs:1209-1295`, `codex-rs/core/src/realtime_conversation.rs:1309-1427`). The config variants are not necessarily trusted local settings because arbitrary client `thread/start.config` values are merged into effective config (`codex-rs/app-server-protocol/src/protocol/v2/thread.rs:63-100`, `codex-rs/app-server/src/config_manager.rs:186-235`). Most decisively, uncapped configured start instructions are rendered verbatim as a developer message on the next ordinary-model turn after its pre-compaction check, so they can materially exceed the model context (`codex-rs/core/src/session/world_state.rs:136-149`, `codex-rs/core/src/context/realtime_start_with_instructions.rs:6-40`, `codex-rs/core/src/session/turn.rs:167-227`). Exploitation is avoidable and requires experimental API/feature opt-in plus an intentionally or erroneously oversized client/config value; realtime is under-development and disabled by default (`codex-rs/app-server-protocol/src/protocol/common.rs:990-995`, `codex-rs/features/src/lib.rs:1561-1565`). |
| H044 | **Plausible** | Frameless context appends are split into independently awaited 500-byte messages; if a later frame fails, core retains the original full text or handoff and the WebRTC sideband reconnect feeds it through the same sender, rebuilding from frame one (`codex-rs/codex-api/src/endpoint/realtime_websocket/methods_frameless_bidi.rs:11,109-125`; `codex-rs/codex-api/src/endpoint/realtime_websocket/methods.rs:439-464`; `codex-rs/core/src/realtime_conversation.rs:1895-1929,1964-1983`; `codex-rs/core/src/realtime_conversation/sideband.rs:131-154`). The append frames carry no per-frame ID or offset, the client consumes no append acknowledgement, and reconnect joins the same call without reinitializing it; therefore any prefix committed before transport loss is appended again (`codex-rs/codex-api/src/endpoint/realtime_websocket/protocol.rs:62-73`; `codex-rs/codex-api/src/endpoint/realtime_websocket/protocol_frameless_bidi.rs:15-34`; `codex-rs/codex-api/src/endpoint/realtime_websocket/methods.rs:863-866,908-918,986-992`). Automatic backend output is capped at about 4,000 bytes, so an eighth-frame failure can replay 3,500 bytes (~875 approximate tokens), while `thread/realtime/appendText` has no semantic text limit; successful reconnect cycles have no outer attempt ceiling, although each handshake is capped and stop/404/410 terminate (`codex-rs/core/src/realtime_conversation.rs:95-96,1450-1460`; `codex-rs/utils/string/src/truncate.rs:71-77`; `codex-rs/app-server-protocol/src/protocol/v2/realtime.rs:315-324`; `codex-rs/core/src/realtime_conversation/sideband.rs:59-99,142-185`; `codex-rs/codex-api/src/endpoint/realtime_websocket/methods.rs:866-895`). |
| H045 | **Plausible** | With the experimental app-server API opted in, the default-off `realtime_conversation` feature enabled, and `flushTranscriptTailOnSessionEnd=true` (also default off), output transcript is retained as role `assistant`; the endpoint test explicitly leaves an assistant-only tail after the last handoff (`codex-rs/features/src/lib.rs:1561-1566`; `codex-rs/app-server-protocol/src/protocol/v2/realtime.rs:197-210`; `codex-rs/codex-api/src/endpoint/realtime_websocket/methods.rs:609-636,1317-1365`). Shutdown checks only enabled/nonempty, adds the generic acknowledgement instruction, and submits the XML as ordinary `UserInput`; an idle thread spawns `RegularTask`, while the close integration test observes a normal Responses request (`codex-rs/core/src/realtime_conversation.rs:1569-1609,1997-2011`; `codex-rs/core/src/session/turn_input.rs:232-314,490-505`; `codex-rs/core/tests/suite/realtime_conversation.rs:4442-4550`). No assistant-role or user-authored-content guard exists—only generic blocking hooks, cancellation, or an active review/compact can prevent dispatch—and accepted input is persisted into normal history for continuity even though paginated app-server history can also persist realtime transcript items (`codex-rs/core/src/session/turn_input.rs:540-635`; `codex-rs/core/src/hook_runtime.rs:600-654`; `codex-rs/core/src/session/mod.rs:4219-4239`; `codex-rs/app-server/src/realtime_event_handling.rs:40-63`). |
| H046 | **Plausible** | Replacement removes the prior state by clearing its active flag and cancelling/awaiting its task, while the only Frameless logical shutdown (`session.close`) is in `RealtimeWebsocketWriter::close`, which this path never calls (`core/src/realtime_conversation.rs:528-542,1095-1110`; `codex-api/src/endpoint/realtime_websocket/methods.rs:411-429`). WebRTC media can begin inference before sideband attachment, and Frameless deliberately reconnects a lost sideband to the same call; the client supplies and owns the peer connection (`codex-api/src/endpoint/realtime_call.rs:136-138`; `core/src/realtime_conversation/sideband.rs:59-166`; `app-server-protocol/src/protocol/v2/realtime.rs:279-290`). Because the old flag is already false, its fanout suppresses `RealtimeConversationClosed` (`core/src/realtime_conversation.rs:1610-1621`), so an unclosed old peer can keep transcription or generation alive until client/service teardown; the claim does not apply when the client proactively closes the old `RTCPeerConnection`, and forcibly ending `ExistingCall` sessions would violate client ownership. |
| H047 | **Plausible** | Under MultiAgentV2, the resident subagent budget is `max_concurrent_threads_per_session - 1`; when full, LRU eviction treats a completed/errored/interrupted nested parent with no active turn or mailbox as unloadable without checking running descendants, then removes it (`codex-rs/core/src/config/mod.rs:1548-1557`; `codex-rs/core/src/agent/control/residency.rs:105-155,233-238`; `codex-rs/core/src/agent/control/spawn.rs:540-543`). A descendant completion targets only the direct parent; delivery resolves only the live thread map, and failure is debug-logged then abandoned with no reload, durable queue, or retry (`codex-rs/core/src/session/mod.rs:2119-2154`; `codex-rs/core/src/thread_manager.rs:1467-1473,1519-1538`). Tests separately establish nested-parent eviction and no fallback after a direct parent disappears (`codex-rs/core/src/agent/control_tests.rs:880-902,3117-3175`). This loses the automatic envelope, not the durable child rollout: manual list/resume/follow-up recovery costs extra model/tool tokens, and after child eviction may require another child inference; the omitted payload can be up to 900 tokens (`codex-rs/core/src/session_prefix.rs:6-9`). |
| H048 | **Plausible** | Legacy full and Last-N agent forks sanitize or truncate response history while retaining surviving `EventMsg` records, including `TokenCount`; only paginated destinations explicitly remove token events (`codex-rs/core/src/agent/control/spawn.rs:63-100`, `codex-rs/core/src/agent/control/spawn.rs:877-879`, `codex-rs/core/src/agent/control/spawn.rs:970-980`). Fork startup then installs the last retained non-`None` usage snapshot unchanged, and active-context accounting starts from its `last_token_usage` rather than recomputing the filtered prompt (`codex-rs/core/src/session/mod.rs:1388-1398`, `codex-rs/core/src/session/mod.rs:1558-1562`, `codex-rs/core/src/context_manager/history.rs:421-455`). Because `run_turn` checks the buffered auto-compact or usable-window limit before recording child input or issuing its first normal sample, a retained near/over-limit parent snapshot plus any estimated retained tail can force unnecessary compaction; Last-N requires such a snapshot to survive its cut, while no-fork and paginated paths are excluded (`codex-rs/core/src/session/turn.rs:155-176`, `codex-rs/core/src/session/turn.rs:1027-1055`, `codex-rs/core/src/session/context_window.rs:23-69`). Duplicate of H073; merge. |
| H049 | **Unlikely** | For an idle MultiAgentV2 target started by one non-parent peer, the initiator is retained for a payload-free completion activity, but the completion envelope is still addressed and queued to the target's direct parent (`codex-rs/core/src/tasks/mod.rs:484-497`; `codex-rs/core/src/session/mod.rs:2073-2149`). The regression test confirms that split: the requester receives `SubAgentActivityKind::Completed`, while `/root` receives the model-visible worker payload (`codex-rs/core/tests/suite/subagent_notifications.rs:2551-2579`, `codex-rs/core/tests/suite/subagent_notifications.rs:2596-2629`). This is explicit hierarchy ownership—the subagent prompt says final output is delivered to its parent (`codex-rs/core/src/session/multi_agents.rs:35-39`)—and `list_agents` exposes completed text (`codex-rs/core/src/agent/control.rs:542-559`; `codex-rs/core/src/tools/handlers/multi_agents_tests.rs:1399-1413`), so polling or relay cost is conditional on a non-parent requester independently needing the payload, not evidence of a general misrouting defect. |
| H050 | **Plausible** | MCP discovery runs every external input schema through a recursive sanitizer that deletes numeric `minimum` and `maximum` from visited properties—including count/limit/page fields when present—and copies the lowered result directly into Responses parameters (`codex-rs/tools/src/mcp_tool.rs:19-55`; `codex-rs/tools/src/json_schema.rs:477-556`; `codex-rs/tools/src/responses_api.rs:157-164`). The loss is meaningful rather than unavoidable: built-in schemas retain and serialize the same keywords, and an outbound request test observes `limit` with `minimum: 1` and `maximum: 32` (`codex-rs/tools/src/json_schema.rs:43-56,135-140`; `codex-rs/core/src/tools/handlers/tool_search_spec.rs:28-39`; `codex-rs/core/tests/suite/search_tool.rs:175-205`). MCP execution only JSON-parses and object-checks arguments before `tools/call`; an enforcing server can reject an advertised-range violation, which is converted to an error tool output and returned on the follow-up model sample (`codex-rs/core/src/mcp_tool_call.rs:132-145,503-565`; `codex-rs/rmcp-client/src/rmcp_client.rs:772-829`; `scripts/mcp_conformance/server.py:210-225,688-700`; `codex-rs/protocol/src/models.rs:2219-2240`; `codex-rs/core/src/stream_events_utils.rs:313-327`; `codex-rs/core/src/session/turn.rs:425-502,558-559`). Corrective extra inference is conditional on server enforcement and descriptions not restating the range, prevalence is unmeasured in checked-in fixtures, and H050 should aggregate with H068 because dynamic tools use the same lossy parser (`codex-rs/tools/src/dynamic_tool.rs:5-14`). |

#### H051-H100

| Hypothesis | Verdict | Justification |
| --- | --- | --- |
| H051 | **Plausible** | Each eligible idle continuation renders the entire template with the full XML-escaped objective and current counters, then starts a synthetic response-item turn (`codex-rs/ext/goal/src/steering.rs:45-77,124-129`; `codex-rs/ext/goal/src/runtime.rs:363-417`). That user message is cloned unchanged into history, appended as a rollout `ResponseItem`, and the next request is built from complete normalized history; resume reconstruction replays those response items (`codex-rs/core/src/hook_runtime.rs:639-658`; `codex-rs/core/src/session/mod.rs:3175-3220`; `codex-rs/core/src/context_manager/history.rs:188-223,479-529`; `codex-rs/core/src/session/turn.rs:371-390`; `codex-rs/core/src/session/rollout_reconstruction.rs:370-384`). The current template is 6,299 chars; its literal body plus the 65-char internal wrapper is 6,292 chars, so 10 copies retain 62,920 static chars and contribute 346,060 logical input chars across requests 1-10; 4,000 ampersands produce a 26,241-byte rendered body (26,306 wrapped) (`codex-rs/ext/goal/templates/goals/continuation.md:1-56`; `codex-rs/ext/goal/src/steering.rs:56-77,124-129`). This requires Goals enabled, durable non-review thread state, an active undeferred idle goal, accepted turn start, successful rollout append, and no compaction/rollback; WebSocket `previous_response_id` can transmit only deltas, so 346K is logical model-context growth, not guaranteed wire bytes (`codex-rs/features/src/lib.rs:1483-1488`; `codex-rs/ext/goal/src/extension.rs:93-107,151-158`; `codex-rs/core/src/client.rs:528-558,1321-1389,1851-1854`). |
| H052 | **Plausible** | Finite goal budgets are enforced only against the goal row for the current thread. Each thread receives fresh `ExtensionData` keyed by its own thread ID; `GoalExtension` installs a per-store `GoalAccountingState`/runtime, activates a turn only from `get_thread_goal(runtime.thread_id())`, and records/persists usage through the current thread/turn stores and `account_thread_goal_usage(self.thread_id(), ...)` (`codex-rs/ext/extension-api/src/state.rs:47-70`; `codex-rs/core/src/session/session.rs:808-810`, `codex-rs/core/src/session/session.rs:1337-1348`; `codex-rs/ext/goal/src/extension.rs:97-115`, `codex-rs/ext/goal/src/extension.rs:196-243`, `codex-rs/ext/goal/src/extension.rs:330-354`; `codex-rs/ext/goal/src/runtime.rs:462-490`; `codex-rs/state/src/runtime/goals.rs:499-610`). Collaboration spawn creates a distinct thread, clones the shared `AgentControl`, and forwards parent/root turn lineage, but neither normal spawn nor its fork initialization supplies the parent goal/accounting state (`codex-rs/core/src/agent/control/spawn.rs:630-693`, `codex-rs/core/src/agent/control/spawn.rs:746-750`, `codex-rs/core/src/agent/control/spawn.rs:1051-1063`; `codex-rs/protocol/src/turn_input.rs:187-203`). Consequently descendant model calls do not increment the root goal. Returned text can be charged only indirectly if a later root inference reports it as non-cached input; automatic V1/V2 completion payloads are bounded below 1,000 approximate tokens, while the child's prompt, intermediate calls, reasoning, and output usage are never transferred (`codex-rs/ext/goal/src/accounting.rs:332-337`; `codex-rs/core/src/agent/control.rs:565-653`; `codex-rs/core/src/session_prefix.rs:9-24`, `codex-rs/core/src/session_prefix.rs:38-54`; `codex-rs/core/src/session_prefix_tests.rs:25-46`). The separate shared rollout budget does aggregate child usage, but it is opt-in/default-off while Goals and Collab are default-on; default V1 permits six open spawned threads, enough to exceed a small root goal budget materially (`codex-rs/core/src/agent/control.rs:111-160`; `codex-rs/core/src/rollout_budget.rs:16-65`; `codex-rs/core/tests/suite/rollout_budget.rs:175-274`; `codex-rs/features/src/lib.rs:1172-1181`, `codex-rs/features/src/lib.rs:1484-1499`; `codex-rs/core/src/config/mod.rs:227-237`, `codex-rs/core/src/config/mod.rs:1529-1560`, `codex-rs/core/src/config/mod.rs:2785-2791`). |
| H053 | **Plausible** | Goal controls are reachable during active work: `Goal` is available during a task while ordinary `/clear` is disabled, and `/goal pause` / `/goal clear` emit only goal-status or goal-clear events (`codex-rs/tui/src/slash_command.rs:208-254`, `codex-rs/tui/src/chatwidget/slash_dispatch.rs:842-888`). Their App handlers only call the goal RPCs; the separate cancellation path explicitly submits `interrupt` and then pauses the goal (`codex-rs/tui/src/app/thread_goal_actions.rs:233-282`, `codex-rs/tui/src/app/event_dispatch.rs:664-675`). Confirmed TUI replacement clears the old goal and then sets an active one (`codex-rs/tui/src/app/thread_goal_actions.rs:181-208,295-308`), causing the API to create a fresh goal with no previous snapshot and a new ID (`codex-rs/ext/goal/src/api.rs:194-238,291-328`, `codex-rs/state/src/runtime/goals.rs:156-203`). Runtime therefore does not satisfy `objective_changed` and injects no objective steer, but it does rebind the still-current turn to the new goal and reset its token baseline (`codex-rs/ext/goal/src/runtime.rs:172-212`, `codex-rs/ext/goal/src/accounting.rs:148-161`). Later token updates and tool finishes account against that new ID while Core can continue the same turn through tool-follow-up samples until normal completion (`codex-rs/ext/goal/src/extension.rs:336-383`, `codex-rs/ext/goal/src/runtime.rs:462-490`, `codex-rs/core/src/session/turn.rs:297-425`). |
| H054 | **Plausible** | A successful `create_goal`, a `get_goal` with an existing goal, and a successful `update_goal` all build the same `GoalToolResponse` around a complete protocol `ThreadGoal`: full objective, thread ID, status, optional budget, cumulative token/time usage, and creation/update timestamps, plus derivable `remainingTokens`; budgeted/timed completion also adds a fixed reporting instruction (`codex-rs/ext/goal/src/tool.rs:174-305`, `codex-rs/ext/goal/src/tool.rs:433-459`, `codex-rs/ext/goal/src/tool.rs:478-489`, `codex-rs/ext/goal/src/tool.rs:515-523`; `codex-rs/protocol/src/protocol.rs:3939-3967`). `JsonToolOutput` stringifies that object as the text of a model-visible `function_call_output`; core records the model's function call before execution and the result after execution, so both enter conversation history and the durable rollout (`codex-rs/tools/src/tool_output.rs:91-145`; `codex-rs/core/src/stream_events_utils.rs:297-327`; `codex-rs/core/src/session/turn.rs:2155-2165`; `codex-rs/core/src/session/mod.rs:3197-3220`). With a 4,000-character unescaped ASCII objective, measured compact result text is 4,244 bytes for budgeted create, about 4,245 bytes for an active get, and 4,246 bytes for blocked update under representative current 10-digit timestamps, so the claimed “about 4,256 bytes” is materially accurate. The duplication is unconditional for create's required objective argument and conditional for get/update on prior goal context surviving; live prompt history also applies configured tool-output truncation and later compaction. |
| H055 | **Plausible** | Goal progress takes an in-memory snapshot, reads the current goal, and executes one `UPDATE ... RETURNING`; query errors return before the local baseline or persisted status advances (`codex-rs/ext/goal/src/runtime.rs:462-523`; `codex-rs/state/src/runtime/goals.rs:499-611`). Turn-stop and tool-finish hooks only warn and return unit on that error (`codex-rs/ext/goal/src/extension.rs:247-271`, `codex-rs/ext/goal/src/extension.rs:363-405`), so task finalization still clears the active turn and invokes thread-idle contributors (`codex-rs/core/src/tasks/mod.rs:796-865`; `codex-rs/core/src/tasks/lifecycle.rs:31-66`). If the failed write leaves the row `active`, `continue_if_idle` rereads it and submits a goal steering item through `start_turn_if_idle`, which starts an automatic `RegularTask` and another model-sampling turn (`codex-rs/ext/goal/src/runtime.rs:363-445`; `codex-rs/core/src/session/turn_input.rs:331-441`; `codex-rs/core/src/tasks/regular.rs:31-85`). Thus a transient turn-boundary write failure can admit at least one full continuation when the missed delta should have crossed budget, while persistent read-success/write-fail has no goal-local retry or circuit breaker. |
| H056 | **Plausible** | The coding-model sample is built from the thread history first (`codex-rs/core/src/session/turn.rs:371-392`). When that model emits `web.run`, core persists the call before executing it and gives the extension a fresh raw-history snapshot (`codex-rs/core/src/stream_events_utils.rs:297-324`; `codex-rs/core/src/tools/handlers/extension_tools.rs:166-218`). The search history builder copies visible user `InputText` unchanged, retains the previous and current user messages, and applies its 1,000-token budget only to assistant `OutputText`; tests assert the exact `[previous user, previous assistant, current user]` shape while excluding images, contextual user messages, and current-turn commentary (`codex-rs/ext/web-search/src/history.rs:10-80,112-204`; `codex-rs/tools/src/response_history.rs:9-70,102-148`). That input, the same model slug, and the commands are serialized into a separate `POST .../alpha/search`; the end-to-end test observes the search request between the first Responses tool call and the second Responses request carrying its function output (`codex-rs/ext/web-search/src/tool.rs:101-147`; `codex-rs/codex-api/src/search.rs:8-29`; `codex-rs/codex-api/src/endpoint/search.rs:31-48`; `codex-rs/app-server/tests/suite/v2/web_search.rs:190-267,293-305`; `codex-rs/ext/web-search/src/output.rs:17-38`). There is no search-local input cap: `max_output_tokens` is output-only. “Uncapped” is therefore scoped, not end-to-end—TUI and app-server submissions are capped at 1,048,576 characters per turn, and provider context limits constrain successful inference—but those controls still permit a near-context-sized second input sample. |
| H057 | **Plausible** | Recaps always start a temporary thread on the current model and submit the structured turn with no effort override; titles use the current model in the fallback path and likewise pass no effort unless the resulting model id is `gpt-5.6-luna`, which is explicitly forced to `low` (`codex-rs/tui/src/app/recap.rs:249-268,360-369`; `codex-rs/tui/src/app/thread_title.rs:42-60,120-130`). Temporary thread creation overrides model/provider/cwd and isolation settings, but neither reasoning effort nor summary; app-server layers those overrides onto normal config, and a checked-in integration test proves a no-effort `thread/start` inherits project-configured `high` (`codex-rs/tui/src/temporary_structured_request.rs:46-49,129-164`; `codex-rs/app-server/src/request_processors/thread_processor.rs:1315-1321,1604-1637`; `codex-rs/app-server/tests/suite/v2/thread_start.rs:884-918`). Core retains configured effort/summary, resolves omitted summary and effort from model metadata, and serializes reasoning independently of the JSON-schema text format, so structured output does not suppress it (`codex-rs/core/src/session/mod.rs:694-718`; `codex-rs/core/src/session/step_settings.rs:45-82,252-264`; `codex-rs/core/src/client.rs:865-881,946-980`; `codex-rs/app-server-protocol/src/protocol/v2/turn.rs:236-239`). Hidden-thread routing drops reasoning deltas, completed reasoning items carry summary/raw content, and the collector ignores every completed item except `AgentMessage`; therefore any generated reasoning or summary is unused by the title/recap consumer (`codex-rs/tui/src/app/app_server_events.rs:95-118`; `codex-rs/app-server-protocol/src/protocol/v2/item.rs:896-899`; `codex-rs/tui/src/temporary_structured_request.rs:200-232`). High/max configured effort and enabled summaries therefore make the claimed hidden-output waste reachable; automatic recap failure can repeat it once for the same turn revision (`codex-rs/tui/src/app/recap.rs:202-211,566-583`). |
| H058 | **Plausible** | `CallToolResult.content` is an uncapped `Vec`, MCP conversion maps every block one-for-one into a structured function-output item, and that payload serializes as a JSON array (`codex-rs/protocol/src/mcp.rs:260-269`; `codex-rs/protocol/src/models.rs:2036-2058,2191-2208,2243-2252,2285-2384`). `McpToolOutput` prepends one wall-time text item and applies the nominal policy times 1.2, but structured truncation debits only text payload bytes/tokens or media/encrypted payload estimates; it never charges per-item tags, keys, quotes, commas, or an item-count allowance (`codex-rs/core/src/tools/context.rs:146-174`; `codex-rs/utils/output-truncation/src/lib.rs:94-199`; `codex-rs/core/src/context_manager/history.rs:669-694`). With the checked-in 10K-token policy, a 33-byte wall-time header costs 9 heuristic tokens and leaves room for 11,991 one-byte text blocks; compact JSON is 395,770 bytes for `output` and 395,829 bytes for a `call1` function-output item, about 386.5 KiB / 98,958 four-byte-heuristic tokens. The byte-policy fallback similarly retains 11,967 blocks and produces 395,037 bytes, so the reported “12K / 384 KiB / 96K” estimate is slightly low but materially correct (`codex-rs/models-manager/models.json:1-18`; `codex-rs/models-manager/src/model_info.rs:145-171`; `codex-rs/utils/string/src/truncate.rs:71-78`). Empty encrypted blocks are stronger: their payload estimate is zero, every block is retained while the positive budget remains unchanged, and each block still serializes to 51 bytes plus array punctuation. History merely repeats the same payload-only truncation; prompt normalization does not coalesce text or rebudget the array, and request construction forwards it as history (`codex-rs/core/src/context_manager/history.rs:218-223,479-507,613-628,696-703,752-769`; `codex-rs/core/src/context_manager/normalize.rs:328-410`; `codex-rs/core/src/session/turn.rs:371-390`; `codex-rs/core/src/client.rs:885-975`). The separate MCP event-size cap truncates only the UI/rollout event copy and returns the original result for model context (`codex-rs/core/src/mcp_tool_call.rs:539-565,892-932`). Aggregate with H078: both have the same missing structured-item framing/count budget; H058 is the MCP one-byte/empty-encrypted witness, while H078 adds zero-duration audio and unsupported-audio expansion. |
| H059 | **Plausible** | The model-facing no-server path follows every ready server's resource cursors into vectors, then sorts and flattens the completed catalogs while explicitly setting `next_cursor: None`; the resulting single JSON string is middle-truncated before the same bounded `FunctionToolOutput` is emitted and returned (`codex-rs/codex-mcp/src/binding_clients.rs:80-105`; `codex-rs/codex-mcp/src/pagination.rs:37-72`; `codex-rs/core/src/tools/handlers/mcp_resource.rs:122-155,280-362`). For a stable plain catalog, retry output is deterministic: server order is canonical, item order is retained, and truncation always preserves half the head and half the tail with only a count marker, so omitted middle descriptors and page boundaries have no continuation handle (`codex-rs/core/src/tools/handlers/mcp_resource_tests.rs:39-64`; `codex-rs/utils/string/src/truncate.rs:7-68,126-152`). A cursor is rejected without an exact server, so recovery within these model tools must restart and fan out through server-specific listings (`codex-rs/core/src/tools/handlers/mcp_resource.rs:76-92`; `codex-rs/core/src/tools/handlers/mcp_resource/list_mcp_resources.rs:73-94`). Generic reads likewise accept only `server` and `uri`, issue one full `resources/read`, and apply the same destructive truncation with no range, cursor, or generic output spill (`codex-rs/core/src/tools/handlers/mcp_resource_spec.rs:60-92`; `codex-rs/core/src/tools/handlers/mcp_resource/read_mcp_resource.rs:63-94`; `codex-rs/core/src/tools/handlers/mcp_resource_tests.rs:160-181`; `codex-rs/core/src/unified_exec/exec_output_artifacts.rs:27-82`). Thus a retry of a stable oversized read can add another near-12K-token result without revealing the middle; the impact is conditional because single-server pagination, typed links, server-defined chunk URIs, and specialized resource readers mitigate subsets. |
| H060 | **Plausible** | A single-server `list_mcp_resources` or `list_mcp_resource_templates` call accepts any nonblank model-supplied cursor and performs one direct page of the respective MCP list method; only the no-server fan-out path uses `collect_paginated`, which owns the 100-page, 2,048-item, 64-KiB-cursor, repeated-cursor, and aggregate-timeout guards (`codex-rs/core/src/tools/handlers/mcp_resource.rs:54-80`; `codex-rs/core/src/tools/handlers/mcp_resource/list_mcp_resources.rs:65-93`; `codex-rs/core/src/tools/handlers/mcp_resource/list_mcp_resource_templates.rs:65-95`; `codex-rs/codex-mcp/src/binding.rs:104-136`; `codex-rs/codex-mcp/src/binding_clients.rs:34-60,78-132`; `codex-rs/codex-mcp/src/pagination.rs:9-80`). Single-server results copy `nextCursor` into model-visible JSON. With GPT-5.6's 10,000-token tool-output policy and the 1.2 serialization allowance, a minimal 40-KiB cursor result is 41,007 bytes, below the resulting 48,000-byte budget, so it survives intact; the next call's roughly 40,988-byte raw arguments are not truncated (`codex-rs/core/src/tools/handlers/mcp_resource.rs:110-132,346-359`; `codex-rs/core/src/tools/handlers/mcp_resource_tests.rs:25-36,149-181`; `codex-rs/models-manager/models.json:4-18`; `codex-rs/protocol/src/protocol.rs:3213-3261`; `codex-rs/protocol/src/models.rs:1037-1095`; `codex-rs/core/src/context_manager/history.rs:481-542`). Model calls and tool outputs are both appended as API history, the next sample is rebuilt from that history, and every tool call requests another sample; therefore a server that keeps returning the same cursor can accumulate repeated result/call pairs with no resource-pagination page or repeat counter (`codex-rs/core/src/stream_events_utils.rs:288-327`; `codex-rs/core/src/session/turn.rs:371-378,397-563,2155-2177,2770-2811`; `codex-rs/core/src/context_manager/history.rs:640-659`; `codex-rs/core/tests/suite/client.rs:2078-2146`). This is conditional on a configured server emitting a pathological cursor and the model reproducing it, but MCP specifies opaque non-null cursors without a byte/repeat ceiling, and Codex's modern HTTP/stdio 8-MiB transport cap does not block 40 KiB (`codex-rs/rmcp-client/src/http_client_adapter.rs:187-196,930-950`; `codex-rs/rmcp-client/src/local_stdio_transport.rs:26-100`). |

| H061 | **Plausible** | Resource handlers serialize MCP resource data into `FunctionToolOutput`, whose `ToolOutput` implementation inherits `contains_external_context() == false`, so the registry pollution hook does not fire (`codex-rs/core/src/tools/handlers/mcp_resource.rs:280-309,348-363`; `codex-rs/core/src/tools/context.rs:217-263`; `codex-rs/tools/src/tool_output.rs:20-24`; `codex-rs/core/src/tools/registry.rs:773-788`). The fallback history/stream checks intentionally exclude function outputs with a call ID, and resource outputs become `call_id: Some(...)`; regular configured MCP calls instead pollute through server metadata (`codex-rs/core/src/session/mod.rs:3218-3227`; `codex-rs/core/src/stream_events_utils.rs:132-157`; `codex-rs/protocol/src/models.rs:1821-1842`; `codex-rs/core/src/mcp_tool_call.rs:453-454,834-850`; `codex-rs/codex-mcp/src/server.rs:409-415`). With MemoryTool enabled, suppression true, generation enabled, and normal startup eligibility, the thread stays `enabled`, while its persisted `FunctionCallOutput` is retained and embedded in phase-1 input subject to normal truncation (`codex-rs/state/src/runtime/memories.rs:148-268`; `codex-rs/rollout/src/policy.rs:43-77`; `codex-rs/memories/write/src/phase1.rs:283-307,404-440,758-780`). |
| H062 | **Plausible** | The single-server resource and template constructors both set a top-level `server` and also map every descriptor through `ResourceWithServer::from_server`; the checked-in resource test explicitly asserts `server == "srv"` at both levels (`codex-rs/core/src/tools/handlers/mcp_resource.rs:104-186`; `codex-rs/core/src/tools/handlers/mcp_resource_tests.rs:14-36,109-119`). The originating function call is retained with its raw `{"server":"srv"}` arguments, while the handler serializes the payload into the plain string of the paired `function_call_output`, so the next request has the semantic shape `function_call(arguments="{\"server\":\"srv\"}")` plus `function_call_output(output="{\"server\":\"srv\",\"resources\":[{\"server\":\"srv\",...}]}")` (`codex-rs/core/src/stream_events_utils.rs:288-327`; `codex-rs/core/src/session/turn.rs:2155-2165`; `codex-rs/core/src/tools/context.rs:217-263,546-569`; `codex-rs/protocol/src/models.rs:815-847,1037-1095,2118-2208`). In compact inner JSON, the removable fragment is exactly `12 + server.len()` bytes per entry: 2,048 copies of `"server":"srv",` are 30,720 bytes (30 KiB), exactly 7,680 tokens under the repository's four-bytes-per-token approximation; `"server":"my-server",` is 43,008 bytes (42 KiB / 10,752 approximate tokens), not 47 KiB (`codex-rs/utils/string/src/truncate.rs:7-78`). The default GPT-5.6 policy becomes a 48,000-byte inner-string allowance after the resource handler's 1.2 multiplier, so a 2,048-entry response is normally middle-truncated rather than adding the whole redundancy beyond the cap; nevertheless the repeated field consumes most of that allowance and displaces descriptors (`codex-rs/models-manager/models.json:4-18`; `codex-rs/protocol/src/protocol.rs:3215-3261`; `codex-rs/core/src/tools/handlers/mcp_resource.rs:346-362`; `codex-rs/core/src/context_manager/history.rs:185-223,479-507,613-634`). The claim must be scoped to explicit single-server calls: the no-server path intentionally flattens descriptors from multiple servers, omits top-level `server`, and therefore needs per-entry identity because `read_mcp_resource` requires both server and URI; removing the field globally would make flattened results ambiguous (`codex-rs/core/src/tools/handlers/mcp_resource.rs:123-128,151-157,179-185`; `codex-rs/core/src/tools/handlers/mcp_resource_tests.rs:38-64,124-146`; `codex-rs/core/src/tools/handlers/mcp_resource_spec.rs:60-89`). |
| H063 | **Plausible** | The rmcp boundary itself does not discard content annotations: rmcp 3.1.3 content blocks serialize their optional `annotations`, and `call_tool_result_from_rmcp` preserves each block, in order, as JSON (`codex-rs/Cargo.toml:408`; `codex-rs/codex-mcp/src/binding.rs:365-379`). The loss occurs in the normal model projection. `convert_mcp_content_to_items` recognizes text/image/audio blocks but models only their payload plus two Codex `_meta` extensions; it has no audience or priority field, and the Responses-compatible output enum has nowhere to carry either annotation (`codex-rs/protocol/src/models.rs:2036-2059,2243-2284,2285-2384`). `McpToolOutput` then prepends a wall-time item and truncates the projected items in original order; the truncator spends its budget sequentially and, once an early text item exhausts it, omits all later text without considering importance (`codex-rs/core/src/tools/context.rs:125-175`; `codex-rs/utils/output-truncation/src/lib.rs:94-224`). Consequently, with absent/null `structuredContent`, an arbitrarily long priority-0 first block can consume the whole remaining output allowance while a priority-1 later block is excluded, and a text block annotated for `audience: ["user"]` becomes ordinary `input_text` in the next Responses request. Existing unit and integration tests establish the same conversion, source-order truncation, and outbound function-output path, though none covers MCP content annotations (`codex-rs/protocol/src/models.rs:3045-3057,3302-3341`; `codex-rs/utils/output-truncation/src/truncate_tests.rs:102-158`; `codex-rs/core/src/tools/context_tests.rs:87-127`; `codex-rs/core/tests/suite/rmcp_client.rs:593-674`). MCP defines these optional annotations as client hints that clients *can* use, not mandatory filtering or access control, so this is not a protocol-conformance or confidentiality failure; it is a conditional context-efficiency/selection defect capable of displacing useful content and inducing a retry. |
| H064 | **Plausible** | The backend `CallToolResult` is immediately consumed by `sanitize_mcp_tool_result_for_model` using the active turn model's modalities; unsupported image/audio blocks are replaced with text objects that omit `data`, `mimeType`, and block metadata (`codex-rs/core/src/mcp_tool_call.rs:423-507,853-889`). That already-sanitized value is then used for both durable paths: the completed MCP turn item/event and the `McpToolOutput` returned to direct tool dispatch (`codex-rs/core/src/mcp_tool_call.rs:539-565,959-1018`; `codex-rs/core/src/tools/handlers/mcp.rs:214-241`). `McpToolOutput` converts only its stored result into a function-call output; the turn loop records that converted item into canonical history and a rollout `ResponseItem` (`codex-rs/core/src/tools/context.rs:100-174`; `codex-rs/protocol/src/models.rs:2223-2384`; `codex-rs/core/src/session/turn.rs:2155-2171`; `codex-rs/core/src/session/mod.rs:3174-3220`). The generic modality filter is correctly request-scoped: each request projects a cloned, copy-on-write history and replaces unsupported media only in that projection (`codex-rs/core/src/session/mod.rs:3875-3878`; `codex-rs/core/src/state/session.rs:115-117`; `codex-rs/core/src/context_manager/history.rs:49-52,207-223,463-476`; `codex-rs/core/src/context_manager/normalize.rs:328-420`; `codex-rs/core/src/session/turn.rs:371-378`). But an MCP call made under a text-only model reaches that projection with only placeholder text already persisted. Cold resume replays persisted `ResponseItem` envelopes and merely prepares media that still exists; event envelopes are not reconstructed into model history, and replay cannot invert a placeholder into the discarded bytes (`codex-rs/core/src/session/rollout_reconstruction.rs:320-337,374-385`; `codex-rs/core/src/session/mod.rs:1458-1505`). Therefore a later image/audio-capable model cannot recover an ordinary media-only MCP result from durable history and needs the tool rerun or an independent external copy. |
| H065 | **Plausible** | A direct MCP dispatch wraps the completed `CallToolResult` in `McpToolOutput`, whose model-facing payload always constructs `Wall time: <seconds> seconds\nOutput:` before truncation (`codex-rs/core/src/tools/handlers/mcp.rs:215-244`; `codex-rs/core/src/tools/context.rs:108-169`). With no structured content and no content entries, conversion first yields an empty content-item array and the header becomes its sole `input_text`; nonempty unstructured results receive the header as the first content item, while structured results receive the header and newline in the output string (`codex-rs/protocol/src/models.rs:2219-2277`; `codex-rs/core/src/tools/context_tests.rs:87-178`, `codex-rs/core/src/tools/context_tests.rs:181-230`). The resulting function output is appended to in-memory history, persisted to rollout, and included in each later logical prompt until a history rewrite; normalization only repairs call/output pairs and strips unsupported media, not this text (`codex-rs/core/src/session/turn.rs:2154-2171`; `codex-rs/core/src/session/mod.rs:3174-3218`; `codex-rs/core/src/context_manager/history.rs:163-230,430-459`; `codex-rs/core/src/session/turn.rs:302-382,397-505`). The repository's four-bytes-per-token heuristic makes the 33-35-byte text prefix plus newline about 9 tokens, or about 17 tokens when represented as a separate 65-67-byte `input_text` object; 100 sequential direct calls therefore retain roughly 0.9-1.7K tokens and create 5,050 header appearances, about 45.5-85.9K gross cumulative input tokens (approximately 90.9K at 18 tokens), consistent with the claimed rounded range (`codex-rs/protocol/src/models.rs:2036-2050`; `codex-rs/utils/string/src/truncate.rs:3,71-83`). |
| H066 | **Plausible** | Provider usage is recorded at response completion as `last_token_usage`; the next admission check uses that prior total plus only raw items after the last model item, with no current-model modality input (`codex-rs/core/src/session/turn.rs:2568-2606`; `codex-rs/core/src/context_manager/history.rs:415-455`). On the next turn, pre-sampling compaction runs before context updates and the new user message, then compares that session-scoped total with the current model's auto-compaction/full-window limits and immediately dispatches compaction when `>=` either boundary (`codex-rs/core/src/session/turn.rs:156-188,1030-1055`; `codex-rs/core/src/session/context_window.rs:23-75`). By contrast, capability normalization is request-scoped and occurs only later on a cloned history: unsupported image/audio content becomes short text and generated-image `result` bytes are cleared (`codex-rs/core/src/context_manager/history.rs:208-224,463-477`; `codex-rs/core/src/context_manager/normalize.rs:328-417`; `codex-rs/core/tests/suite/model_switching.rs:865-991,1083-1187`). No model-switch path recomputes usage before the trigger; the available recomputation runs only when usage is absent or after history rewrites and estimates raw, unnormalized history (`codex-rs/core/src/session/mod.rs:4076-4147`; `codex-rs/core/src/session/turn.rs:2568-2606`). Equal-window models with equal or absent compaction hashes bypass previous-model compaction, so a prior total `U >= L` still reaches the generic trigger even when removed media `S` exceeds pending additions `A` plus the overage, making the actual next request `U - S + A < L` (`codex-rs/core/src/session/turn.rs:1059-1188`; `codex-rs/protocol/src/openai_models.rs:500-521`). Existing tests separately prove next-turn pre-compaction from prior reported usage and multimodal-to-text request shrink, making the crossing reachable though not covered in one combined test (`codex-rs/core/tests/suite/compact.rs:1993-2060,4881-4971`; `codex-rs/core/tests/suite/model_switching.rs:865-991`). |
| H067 | **Plausible** | Current OpenAI documentation makes GPT-5.6 Sol, Terra, and Luna patch-based for every detail level: after detail-specific resizing, billable image tokens are `ceil(ceil(width/32) * ceil(height/32) * 1.2)`. Codex instead replaces every eligible non-`original` inline image with 7,373 heuristic bytes, or 1,844 tokens, while `original` uses the unmultiplied 32-pixel patch count capped at 10,000 (`codex-rs/core/src/context_manager/history.rs:707-717,820-863`). The cited preparation limits and tests establish the transmitted dimensions: 64x32 stays 64x32, high-detail 2048x2048 becomes 1600x1600, original 2304x864 stays unchanged, and an over-budget square is reduced to 3200x3200 (`codex-rs/core/src/image_preparation.rs:34-42,318-346`; `codex-rs/core/src/image_preparation_tests.rs:37-68,106-160,172-205`; `codex-rs/core/src/context_manager/history_tests.rs:2698-2766`). Recalculation therefore confirms every claimed delta: 64x32 is estimated as 1,844 versus `ceil(2*1*1.2)=3` (+1,841); 1600x1600 is 1,844 versus `ceil(50*50*1.2)=3,000` (-1,156); 2304x864 original is 1,944 versus `ceil(72*27*1.2)=2,333` (-389); and a prepared 10,000-patch original is 10,000 versus 12,000 (-2,000). GPT-5.6 catalog entries use Responses Lite, which strips image detail before transmission; omitted detail defaults to `auto`, and current GPT-5.6 `auto` uses the same sizing behavior as `original`, so this does not restore agreement (`codex-rs/core/src/client_common.rs:55-85`; `codex-rs/models-manager/models.json:4-31,134-162,260-288`). The discrepancy is behaviorally reachable: post-sampling context checks add locally produced tool outputs with this estimator and compact before a required follow-up when the threshold is crossed, so a 64x32 tool image can contribute 1,841 phantom tokens and cause premature compaction (`codex-rs/core/src/context_manager/history.rs:416-455`; `codex-rs/core/src/session/context_window.rs:23-79`; `codex-rs/core/src/session/turn.rs:413-472`). Guardian also rejects the whole image set when the estimated prompt exceeds 10,000 tokens; six 64x32 images contribute at least 11,064 estimated image tokens although their documented GPT-5.6 charge is only 18, demonstrating failed admission (`codex-rs/core/src/guardian/review_session.rs:92,1043-1095`). |
| H068 | **Plausible** | App-server retains each dynamic tool's original `input_schema` as arbitrary JSON and only checks that the shared parser accepts it, but the model-facing copy is reparsed into the limited `JsonSchema` representation: string/array bounds have no representable fields, inclusive numeric bounds are deliberately deleted, and JSON-Schema boolean forms are rewritten to `{"type":"string"}` before the result is serialized as Responses parameters (`codex-rs/protocol/src/dynamic_tools.rs:11-31`; `codex-rs/app-server/src/request_processors/thread_processor.rs:281-367,1408-1454`; `codex-rs/tools/src/json_schema.rs:39-77,199-220,477-556`; `codex-rs/tools/src/dynamic_tool.rs:5-14`; `codex-rs/tools/src/responses_api.rs:82-87,157-164`). The TUI's direct dynamic transport supplies concrete mismatches after filtering out its approval-gated delegation tools: it advertises bounded integers, a nonempty title, and a 1-8-item array, while its client-owned executor rejects `limit: 0`, `title: ""`, and zero/more-than-eight targets and converts the error text into a `success: false` response (`codex-rs/tui/src/dynamic_tools.rs:146-225,252-268,271-300,426-496,783-786,830-839`; `codex-rs/tui/src/dynamic_tools_mcp.rs:58-69`; `codex-rs/tui/src/app/app_server_events.rs:323-412`). Core itself only JSON-parses and delegates the call; it records the returned output and unconditionally schedules another model sample after a tool call, and app-server integration tests observe both ordinary and error text in that follow-up request (`codex-rs/core/src/tools/handlers/dynamic.rs:125-164`; `codex-rs/core/src/stream_events_utils.rs:297-327`; `codex-rs/core/src/session/turn.rs:397-425,502-563,2155-2173,2420-2426,2780-2786`; `codex-rs/app-server/tests/suite/v2/dynamic_tools.rs:339-571,808-859`). Primitive `{"type":"boolean"}` is preserved; only boolean-schema forms change, and no checked-in TUI tool exercises an original `false` schema, so that subcase is conditional. H050 is the same lowering root for MCP numeric bounds and should be merged rather than counted independently. |
| H069 | **Unlikely** | The code does deliberately reuse routing keys across distinct threads: ordinary API-key root/child requests use the shared session ID, and Guardian trunk/ephemeral sessions use `guardian:<parent_thread_id>` (`codex-rs/core/src/client.rs:514-526`; `codex-rs/core/tests/suite/prompt_cache_key.rs:92-155`; `codex-rs/core/src/guardian/review_session.rs:298-310`; `codex-rs/core/src/guardian/tests.rs:3421-3428`). Responses Lite also derives deterministic prefix-item IDs from both the thread ID and payload, so equal tools/instructions in different threads get different IDs (`codex-rs/core/src/client.rs:894-928`; `codex-rs/core/src/client_tests.rs:340-390`). Those IDs are serialized to the final HTTP/WebSocket request, not stripped (`codex-rs/core/tests/suite/responses_lite.rs:124-153`; `codex-rs/core/tests/suite/client_websockets.rs:2020-2047`). The claimed cache loss, however, does not follow: OpenAI's current official caching semantics match the model's full **rendered context**, route partly by a hash of initial **tokens**, and cache token-derived KV tensors—not raw request JSON. The API reference describes `additional_tools.id` only as an optional unique item identifier, and no cited test compares `cached_tokens` or `cache_write_tokens` for requests differing only by IDs. The ID change was introduced specifically so WebSocket continuation can recognize unchanged items and send incremental input, not as rendered prompt content. Without provider evidence that these metadata IDs enter rendering or cache matching, different wire IDs do not establish different cache prefixes. |
| H070 | **Plausible** | Direct parallel tool calls establish the claimed overlapping-prefix mechanism. For the kth emitted call, Codex first persists that `FunctionCall`/`CustomToolCall` in parent history and then invokes `handle_tool_call`; that non-async constructor immediately spawns the dispatch task, while parallel-capable tools such as `exec_command` take a shared read lock and can reach approval concurrently (`codex-rs/core/src/stream_events_utils.rs:297-327`; `codex-rs/core/src/tools/parallel.rs:73-88,91-180`; `codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs:98-106`; `codex-rs/core/src/tools/orchestrator.rs:178-227`). Consequently, the kth Guardian prompt snapshot cannot precede persistence of calls 1..k and contains at least that prefix, even if scheduling delays make it contain more. Every synchronous Guardian attempt independently clones the live parent history, renders nonempty call arguments as tool transcript entries, and appends the reviewed action again as planned-action JSON (`codex-rs/core/src/guardian/prompt.rs:142-175,185-216,300-325,407-500,525-598`). A busy reusable trunk sends each later concurrent review to its own ephemeral fork from the last committed trunk snapshot; the checked-in concurrency test proves the fork sees newer parent-history entries, excludes the still-in-flight trunk result, and is recreated after a parse failure (`codex-rs/core/src/guardian/review_session.rs:509-645,768-815,825-915`; `codex-rs/core/src/guardian/tests.rs:3206-3468`). Thus N overlapping reviews can expose triangular duplicated call material in the unsaturated range. The limits materially qualify the headline: tool entries are capped at 1,000 estimated tokens each, tool transcript at 10,000 per review, and retained non-user entries at 40, so growth becomes linear after saturation; ten exactly-full 1,000-token payloads can total about 54K rather than a strict 55K because rendered labels also consume the 10K budget, although ten near-1K entries can realize roughly 55K and the ten planned-action copies add further duplication (`codex-rs/core/src/guardian/mod.rs:69-75`; `codex-rs/core/src/guardian/prompt.rs:417-500`; `codex-rs/utils/string/src/truncate.rs:64-78`). The stated ceilings are otherwise supported: Code Mode permits 1,024 unresolved delegate callbacks per host connection and dispatches invoke callbacks into separate tasks, while synchronous Guardian uses at most three outer attempts under one 90-second deadline and retries only parse errors or enumerated transient session failures (`codex-rs/code-mode-protocol/src/host/mod.rs:12-13`; `codex-rs/code-mode-host/src/peer.rs:123-155`; `codex-rs/core/src/tools/code_mode/delegate.rs:117-190`; `codex-rs/core/src/guardian/review.rs:78,486-493,1042-1112`; `codex-rs/core/src/guardian/tests.rs:2896-3041`). |

| H071 | **Plausible** | Each streamed tool-call `OutputItemDone` is appended to canonical history and then immediately constructs `handle_tool_call`; that non-async constructor immediately `tokio::spawn`s dispatch, and parallel-capable `exec_command` runs matching hook logic inside that task (`codex-rs/core/src/stream_events_utils.rs:297-327`; `codex-rs/core/src/tools/parallel.rs:113-175`; `codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs:98-100,415-460`). A matching synchronous `PreToolUse` hook can emit `additionalContext`, which is recorded directly before execution, while a successful `PostToolUse` can do the same after execution; both bypass the ordered tool-output drain (`codex-rs/core/src/tools/registry.rs:568-577,683-705`; `codex-rs/core/src/hook_runtime.rs:202-210,764-775`; `codex-rs/core/tests/suite/hooks.rs:3385-3443,4754-4820`). Therefore, if the server pauses after call A's completed item but before a later response item, the reachable history order is `prior input, call A, hook context, call B, ..., outputs`; history recording appends in lock-acquisition order and prompt normalization does not reorder already-paired calls, messages, or outputs (`codex-rs/core/src/session/mod.rs:3174-3200`; `codex-rs/core/src/context_manager/history.rs:188-204,459-476`). Meanwhile the WebSocket tracker independently retains only server-returned items in server order, so its baseline is `prior input, call A, call B, ...` (`codex-rs/core/src/client.rs:2167-2224`). The same turn-scoped client session then rebuilds the follow-up from local history; its exact prefix comparison fails at the inserted hook message, and the WebSocket fallback sends `previous_response_id: None` with the full request input (`codex-rs/core/src/session/turn.rs:295-303,371-390`; `codex-rs/core/src/client.rs:1311-1390,1833-1854`). Existing tests prove tools can finish before `response.completed`, prove hook context reaches the immediate follow-up, and prove prefix/non-prefix WebSocket requests respectively use `previous_response_id` or replay full input (`codex-rs/core/tests/suite/tool_parallelism.rs:304-428`; `codex-rs/core/tests/suite/client_websockets.rs:2181-2284`). |
| H072 | **Plausible** | A direct model tool call is persisted before execution and unconditionally sets `needs_follow_up`; queued execution maps both a handler `FunctionCallError::Fatal` and a spawned handler `JoinError` to `CodexErr::Fatal` (`codex-rs/core/src/stream_events_utils.rs:288-327`; `codex-rs/core/src/tools/parallel.rs:73-88,140-180,211-214`; `codex-rs/core/src/session/mod.rs:3174-3220`). After the response stream ends, `drain_in_flight` records successful outputs, but for every error it only calls `error_or_panic`, keeps draining, and then unconditionally returns `Ok(())`. Debug builds panic there; release builds merely log, so the already-computed successful `SamplingRequestResult { needs_follow_up: true, .. }` survives unchanged (`codex-rs/core/src/session/turn.rs:2155-2178,2249-2252,2420-2426,2609-2619,2770-2811`; `codex-rs/core/src/util.rs:93-98`). `run_sampling_request` consequently never sees the non-retryable fatal, and `run_turn` takes its success path and continues to another sampling pass (`codex-rs/protocol/src/error.rs:380-415`; `codex-rs/core/src/session/turn.rs:397-563,1405-1452`). The call is durable but its output is absent, so the next request's cloned history is normalized by inserting a synthetic `"aborted"` output immediately after the call; tests establish both insertion and a stable repeated prompt projection (`codex-rs/core/src/session/turn.rs:371-378`; `codex-rs/core/src/context_manager/history.rs:207-223,463-477`; `codex-rs/core/src/context_manager/normalize.rs:21-136`; `codex-rs/core/src/context_manager/history_tests.rs:1659-1697,1934-1970,1986-2025`). This is not hypothetical: standalone web search maps provider, auth, and request failures to `Fatal`, memories maps backend I/O failures to `Fatal`, external clock tools map provider failures to `Fatal`, and any spawned handler panic becomes the fatal join error (`codex-rs/ext/web-search/src/tool.rs:98-147`; `codex-rs/ext/memories/src/tools/mod.rs:95-112`; `codex-rs/core/src/tools/handlers/current_time.rs:88-104`; `codex-rs/core/src/tools/handlers/sleep.rs:111-143`). The release-only extra inference is therefore real when the model response otherwise completes and the turn is not cancelled. The “full-context” qualifier is conditional rather than intrinsic: a reusable Responses websocket can send only the synthetic appended delta, while HTTP or failed incremental matching sends the full prompt (`codex-rs/core/src/client.rs:1311-1390,1817-1854`). |
| H073 | **Plausible** | Legacy agent forks load the parent's persisted rollout, then sanitize or truncate model-visible response history while retaining `EventMsg` records; `TokenCount` is persisted, and only paginated fork destinations explicitly remove it (`codex-rs/rollout/src/policy.rs:88-116`; `codex-rs/core/src/agent/control/spawn.rs:63-100,126-145,857-981`). The resulting vector is passed as `InitialHistory::Forked`; startup reconstructs the smaller history and then restores the latest non-`None` parent `TokenUsageInfo` unchanged (`codex-rs/core/src/agent/control/spawn.rs:1054-1070`; `codex-rs/core/src/session/mod.rs:1388-1398,1558-1562`). Active-context accounting subsequently starts from that snapshot's `last_token_usage.total_tokens`, adds estimated retained items after the last model-generated item, and tests the total before recording child input or issuing the child's first normal sample (`codex-rs/core/src/context_manager/history.rs:418-455`; `codex-rs/core/src/session/context_window.rs:23-69`; `codex-rs/core/src/session/turn.rs:155-176,1027-1055`). A materially filtered child can therefore make an unnecessary compaction call from stale near-limit parent accounting. Exact duplicate of H048; merge. |
| H074 | **Plausible** | Local rollout writes are atomic only per append: `AppendItems` takes the per-thread writer lock, queues its records, and flushes before releasing it, while the context and compaction paths issue separate calls for model-visible history or `Compacted`, full `WorldState`, and `TurnContext`; failures are only logged by `Session::persist_rollout_items` (`codex-rs/thread-store/src/local/live_writer.rs:309-364`; `codex-rs/core/src/session/mod.rs:3518-3550,3861-3873,3988-4061`). A crash, persistent write failure, cancellation, or fork between calls can therefore expose a complete durable prefix. Reconstruction replays durable `ResponseItem`s and replacement history, but a missing post-compaction `TurnContext` leaves the reference baseline cleared, and a pre-user-message `TurnContext` is not accepted as a baseline unless its segment later contains a real user-turn boundary; the next context update sees no reference and injects full initial context again (`codex-rs/core/src/session/turn.rs:226-269`; `codex-rs/core/src/context_manager/history.rs:1014-1024`; `codex-rs/core/src/session/rollout_reconstruction.rs:30-140,185-337,370-509`; `codex-rs/core/src/session/tests.rs:2710-2773`). This affects initial-context setup and compactions that themselves injected initial context; a missing `WorldState` alone does not establish the full-duplication impact. Separately, local compaction durably records every `OutputItemDone` before receiving `Completed`; a later terminal stream error returns without a `Compacted` replacement, so failed-attempt output remains in live and resumed history, while manual `/compact` suppresses non-abort errors and an over-limit automatic failure can be requested again on the next turn because the failed attempt never reaches token-usage recomputation (`codex-rs/core/src/compact.rs:245-398,735-806`; `codex-rs/core/src/tasks/compact.rs:21-84`; `codex-rs/core/src/session/turn.rs:164-190,1028-1058`). Finally, interruption deliberately flushes the model-visible marker, runs hooks, and only then appends and flushes `TurnAborted`; an interrupted fork observing that marker-only prefix still classifies the explicit lifecycle turn as in progress, and `append_interrupted_boundary` unconditionally adds a second marker before its synthetic abort (`codex-rs/core/src/tasks/mod.rs:901-995`; `codex-rs/core/src/thread_manager.rs:988-1018,2205-2334`; `codex-rs/app-server/src/request_processors/thread_processor.rs:4754-4774,4889-4963`). Thus the three impacts are reachable, although by different conditions: context/checkpoint duplication needs a later resume or fork and is a snapshot/crash/control-flow consistency issue, failed compaction output needs only an ordinary stream failure, and duplicate interruption text needs a fork interleaving (plain resume alone does not synthesize it). |
| H075 | **Plausible** | A reachable bounded V2 fork can expose an exact duplicate of current context. On a parent turn after an approval-policy/profile change, `record_context_updates_and_set_reference_context_item` appends the new full `<permissions instructions>` fragment before recording that turn's user message; checked-in request tests confirm policy changes append a new permissions item (`codex-rs/core/src/session/turn.rs:208-285`; `codex-rs/core/src/session/mod.rs:3993-4055,4226-4236`; `codex-rs/core/tests/suite/permissions_messages.rs:415-467`). If that turn is the later of two retained turns, `fork_turns: "2"` starts the suffix at the preceding real user boundary, so the newer contextual developer item survives; contextual messages are not fork boundaries, and Last-N then deliberately drops every `TurnContext`/`WorldState` record while retaining developer/user response messages (`codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs:113-223,298-323`; `codex-rs/core/src/event_mapping.rs:64-100`; `codex-rs/core/src/thread_rollout_truncation.rs:257-278`; `codex-rs/core/src/agent/control/spawn.rs:63-99,852-1017`). With no model/role override, child construction copies the spawning turn's approval policy, permission profile, cwd, and selected environments, so its first turn builds the same current permissions fragment; reconstruction has restored the retained response item but no typed baseline, causing unconditional full-context injection (`codex-rs/core/src/tools/handlers/multi_agents_common.rs:169-214,230-267`; `codex-rs/core/src/tools/handlers/multi_agents_tests.rs:4380-4470`; `codex-rs/core/src/session/rollout_reconstruction.rs:117-131,324-429,457-509`; `codex-rs/core/src/session/mod.rs:1388-1435,1460-1512,3993-4015`). Both copies reach the model: prompt construction clones all history, and normalization only repairs tool call/output pairs and strips unsupported media; it performs no message or contextual-fragment deduplication (`codex-rs/core/src/session/turn.rs:367-383`; `codex-rs/core/src/context_manager/history.rs:207-223,459-477`). Mid-turn compaction inside the retained suffix is a stronger `fork_turns: "1"` variant because its retained replacement history can already contain the full initial-context bundle while the associated baselines are still removed (`codex-rs/core/src/compact.rs:64-106`; `codex-rs/core/src/compact_remote.rs:337-350`; `codex-rs/core/src/agent/control/spawn.rs:977-1017`). This is conditional on a retained later context update or checkpoint, but it is distinct from BUGS.md #11's FullHistory reconstruction failure. |
| H076 | **Plausible** | `TokenBudget` is under-development and default-off, and model catalog defaults do not enable it, but an explicitly enabled pristine thread can run `Op::Compact` before any user turn (`codex-rs/features/src/lib.rs:1490-1494`; `codex-rs/core/tests/suite/token_budget.rs:537-571,1086-1129`). That manual path emits `TurnStarted` without a user boundary, builds the complete initial-context bundle, and persists a replacement-history checkpoint followed by a full `WorldState` and `TurnContext` (`codex-rs/core/src/compact_token_budget.rs:26-44,79-84`; `codex-rs/core/src/session/mod.rs:3496-3542,3918-3965`). Reverse reconstruction sees the later `TurnContext` first and records `Latest`; the checkpoint therefore does not turn it into `Cleared`, but finalization promotes `Latest` only for a segment that counts as a user turn. It still installs the checkpoint's replacement history, so when no older surviving user-turn baseline exists, resume or a completed full-history thread fork reconstructs the full bundle with a `None` reference baseline (`codex-rs/core/src/session/rollout_reconstruction.rs:80-135,189-223,252-345,370-374,457-467`; `codex-rs/core/src/session/mod.rs:1338-1524`; `codex-rs/core/src/thread_manager.rs:2262-2292`). The next real turn treats that missing baseline as requiring full context, appends another complete bundle, and sends both because history recording and prompt normalization do not deduplicate contextual messages (`codex-rs/core/src/session/mod.rs:3993-4041`; `codex-rs/core/src/session/turn.rs:208-276,371-390`; `codex-rs/core/src/context_manager/history.rs:186-224,463-477`). The full duplicate is conditional: an older surviving user-turn `TurnContext` prevents full reinjection, and inline auto-compaction belongs to an active user turn. This is the same baseline-promotion root as BUGS.md #11 rather than an independent fork defect; H075 is distinct because Last-N truncation deliberately drops `TurnContext`/`WorldState`. |
| H077 | **Plausible** | The bundled GPT-5.6, GPT-5.5, GPT-5.4, and GPT-5.4-mini entries use a nominal 10,000-token tool-output policy; history first multiplies that to 12,000, then the shared truncator treats each token as four UTF-8 bytes, so a text output may retain 48,000 payload bytes plus its marker (`codex-rs/models-manager/models.json:4-31,134-162,260-288,612-640,728-756,845-873`; `codex-rs/core/src/context_manager/history.rs:479-508`; `codex-rs/protocol/src/protocol.rs:3213-3262`; `codex-rs/utils/string/src/truncate.rs:4,15-34,71-83`). OpenAI's official `tiktoken` 0.14.0 maps every `gpt-5*` name to `o200k_base`; reproducing the Rust truncator locally with that encoder measured 48,028 retained bytes as 32,742 tokens for deterministic random Base64, 32,786 for a conventional hex dump, and 36,084 for Base85. The checked-in `seq 1 100000` exec shape remains about 40.2 KB because exec applies its own 10K heuristic before the serialization allowance, but still measured 21,724 tokens; dumping the real `codex-rs/Cargo.lock` through that same reachable shape measured 15,131 (`codex-rs/core/src/tools/context.rs:440-457,509-533`; `codex-rs/core/src/unified_exec/mod.rs:74-76,216-217`; `codex-rs/core/tests/suite/truncation.rs:208-279,584-630`). These are material undercounts even against the effective 12K allowance, not merely the known 1.2 multiplier. The claimed tokenizer worst case is also real: a 48,000-byte valid string assembled from supplementary private-use Unicode scalars that individually remain byte-fallback tokens encoded to exactly 48,000 `o200k_base` tokens, although that witness is contrived and is not needed for the verdict. MCP and ordinary function outputs can reach the full 48 KB history path, are persisted, and are sent in the required follow-up request; until that request reports usage, context admission charges the local output with the same bytes/4 estimator (`codex-rs/core/src/tools/context.rs:145-175,217-258`; `codex-rs/core/src/session/mod.rs:3174-3220`; `codex-rs/core/src/context_manager/history.rs:434-449,613-629,700-702`; `codex-rs/core/src/session/turn.rs:372-390,2155-2173,2780-2786`). The 1 MiB raw-output cap and the models' 272K-or-larger context caps do not enforce a 10K per-output limit, so they do not defeat the finding (`codex-rs/core/src/unified_exec/mod.rs:74-76`; `codex-rs/models-manager/models.json:30-31,161-162,287-288,639-640,755-756,872-873`). |
| H078 | **Plausible** | Structured-output truncation debits text payload length or the media/encrypted estimator, but never each item's JSON tag, keys, quotes, comma, or an item-count allowance (`codex-rs/utils/output-truncation/src/lib.rs:94-222`; `codex-rs/protocol/src/models.rs:2036-2058,2188-2208`). With the checked-in 10K-token policy and the history layer's 1.2 allowance, 12,000 one-byte content items fit the 12K charged budget yet serialize as a 396,060-byte function-output item, about 99,015 four-byte-heuristic tokens; the MCP wall-time header leaves 11,991 blocks and 395,829 bytes / 98,958 heuristic tokens (`codex-rs/models-manager/models.json:4-18`; `codex-rs/core/src/context_manager/history.rs:479-507`; `codex-rs/core/src/tools/context.rs:145-174`; `codex-rs/utils/string/src/truncate.rs:71-78`). That framing arm is the same root cause and witness as H058, not a separate defect. The audio arm is independently reachable and stronger: dynamic-tool responses expose an uncapped content-item `Vec`, accept inline audio, and map every item one-for-one into function-output content (`codex-rs/app-server-protocol/src/protocol/v2/item.rs:1646-1663`; `codex-rs/app-server/src/dynamic_tools.rs:36-53,58-97`; `codex-rs/core/src/tools/handlers/dynamic.rs:153-164`; `codex-rs/protocol/src/models.rs:2095-2112`). A valid one-sample 8 kHz PCM WAV passes preparation, has duration 1/8000 second, and is charged one token; therefore one current 10K-policy output retains 12,000 such clips (`codex-rs/utils/audio/src/lib.rs:107-116,149-206,229-259,304-349`; `codex-rs/core/src/session/mod.rs:3067-3099,3175-3208`; `codex-rs/core/src/context_manager/history.rs:613-628,673-690`). For a model without audio input, prompt normalization then replaces every retained clip with the 60-byte unsupported-audio text while preserving 12,000 separate array items and never reruns output truncation (`codex-rs/core/src/context_manager/history.rs:218-223,463-480`; `codex-rs/core/src/context_manager/normalize.rs:379-415`; `codex-rs/core/src/context/unsupported_media.rs:16-18`; `codex-rs/core/src/client_common.rs:55-64`; `codex-rs/core/src/client.rs:894-990`). The resulting single item is 1,104,060 compact-JSON bytes, about 276,015 heuristic tokens. Before replacement, full-history accounting discounts each base64 payload to its one-token duration estimate and sees only about 192,015 tokens, so post-sampling rollover can remain below the 244,800 auto-compact threshold of a 272K model and the expanded request is built without final rebudgeting (`codex-rs/core/src/context_manager/history.rs:418-455,696-776,790-809,912-947`; `codex-rs/protocol/src/openai_models.rs:499-522`; `codex-rs/core/src/session/context_window.rs:23-69`; `codex-rs/core/src/session/turn.rs:413-480,1385-1404`). A zero-sample PCM WAV is also accepted, computes exactly zero duration/tokens, and the `cost <= remaining_budget` branch retains every such item without reducing the budget; overall history/context compaction can eventually constrain what reaches inference, but the per-output retention path is genuinely count-unbounded. Merge H078's framing portion into H058 while retaining the unsupported-audio rebudgeting witness. |
| H079 | **Plausible** | The user-facing `tool_output_token_limit` is an `Option<usize>` and its schema has a zero minimum but no maximum, so negative user configuration is rejected rather than wrapped, while intentionally very large nonnegative limits are accepted and copied into model-manager overrides (`codex-rs/config/src/config_toml.rs:304-305`; `codex-rs/core/config.schema.json:6578-6583`; `codex-rs/core/src/config/mod.rs:1589-1603,4155-4166`). Overrides preserve the catalog mode and convert the token budget to an `i64`, saturating oversized values at `i64::MAX`; the checked-in 100,000-token byte-mode test confirms that a high setting raises retained output to about 400-401 KB (`codex-rs/models-manager/src/model_info.rs:26-50`; `codex-rs/utils/string/src/truncate.rs:71-78`; `codex-rs/core/tests/suite/truncation.rs:50-123`). The unsafe path is catalog metadata: `TruncationPolicyConfig.limit` is signed `i64` with no nonnegative validation, and `model_catalog_json` deserializes a full catalog then checks only that it is nonempty (`codex-rs/protocol/src/openai_models.rs:335-382,738-835`; `codex-rs/core/src/config/mod.rs:2026-2055,3899-3903,4208-4212`; `codex-rs/core/src/config/config_tests.rs:9389-9443`). A configured OpenAI/custom provider installs that catalog as authoritative, matching model metadata is copied unchanged when no explicit tool-output override exists, and runtime conversion uses unchecked `config.limit as usize` (`codex-rs/core/src/thread_manager.rs:367-375`; `codex-rs/model-provider/src/provider.rs:312-320,435-447`; `codex-rs/models-manager/src/manager.rs:207-218,732-753`; `codex-rs/protocol/src/protocol.rs:3212-3262`). On the current 64-bit targets, `-1_i64 as usize` is `usize::MAX`; token-to-byte saturation and the 1.2 serialization multiplier keep the effective budget huge. Direct MCP output and subsequent history recording both apply that policy, so feasible text is returned and retained without this per-output truncation (`codex-rs/core/src/tools/context.rs:99-175`; `codex-rs/core/src/context_manager/history.rs:165-205,479-510,611-627`; `codex-rs/utils/output-truncation/src/lib.rs:17-30,94-161`). Thus negative signed catalog values concretely disable the truncation policy; high user settings can do so intentionally. |
| H080 | **Plausible** | With `ImageResizeNotice` enabled, image preparation runs at the durable history boundary before the context manager applies its tool-output retention policy (`codex-rs/core/src/session/mod.rs:3065-3095,3174-3208`). For a tool output, preparation records every successfully resized image that survives the separate 10,000-patch preparation budget, builds one complete `ImageResizeNotice`, and appends that standalone developer message immediately after the output (`codex-rs/core/src/image_preparation.rs:34-42,107-130,181-186,238-303`; `codex-rs/core/src/context/image_resize_notice.rs:35-77`). History then processes those adjacent items independently: `FunctionCallOutput` and `CustomToolCallOutput` payloads pass through `truncate_function_output_payload`, which retains at most four images, at most 8 MiB of encoded image URLs, and only images that fit the remaining byte/token budget, while the following `Message` notice is cloned unchanged (`codex-rs/core/src/context_manager/history.rs:188-203,479-535,613-629`; `codex-rs/utils/output-truncation/src/lib.rs:12-15,94-170,203-224`). The next model request clones that history and `for_prompt` only repairs call/output relationships and strips unsupported media; it does not reconcile notice lines with retained images, so stale lines remain model-visible (`codex-rs/core/src/context_manager/history.rs:207-223,459-476`; `codex-rs/core/src/session/turn.rs:371-378,1386-1404`). Persistence is also real: the pre-history-processing prepared output and its notice are both written to the rollout, while reconstruction reapplies output truncation, disables creation of duplicate resize notices, and retains the recorded notice message (`codex-rs/core/src/session/mod.rs:1474-1508,3191-3228`; `codex-rs/core/src/session/rollout_reconstruction.rs:373-393`). A concrete producer exists: dynamic tool responses accept an unconstrained vector of image content items and convert it to `FunctionToolOutput` without pre-truncation (`codex-rs/protocol/src/dynamic_tools.rs:58-73`; `codex-rs/core/src/tools/handlers/dynamic.rs:153-164`; `codex-rs/core/src/tools/context.rs:217-260`). Five 2304x864 images are a straightforward witness: preparation resizes each to 2048x768, five such images consume 7,680 of the shared 10,000 patches, and the later four-image cap necessarily drops at least the fifth while its notice line survives; six consume 9,216 patches and leave at least two stale lines. A large leading text segment or the encoded-byte limit can cause still more of the already-noticed images to be removed. The impact is bounded and opt-in rather than broad: normal screenshots usually yield only a handful of stale lines, the preparation patch budget caps the notice population, some MCP/code-mode producers pre-truncate before this boundary, and `ImageResizeNotice` is under development and disabled by default (`codex-rs/core/src/tools/context.rs:129-175`; `codex-rs/features/src/lib.rs:1369-1374`; `codex-rs/features/src/tests.rs:26-34`). |

| H081 | **Plausible** | Reverse reconstruction records a checkpoint in the active segment, but immediately advances `rollout_suffix` past it and globally records that compaction was seen; only later does `finalize_active_segment` learn that rollback discards that user-turn segment, returning before transferring its replacement history (`codex-rs/core/src/session/rollout_reconstruction.rs:90-115,187-227`). This is reachable for inline compaction because a regular turn persists `TurnStarted` before `run_turn`, pre-turn compaction runs before that turn's context/user records, and mid-turn compaction can run after the user boundary while the same turn continues (`codex-rs/core/src/tasks/regular.rs:47-77`; `codex-rs/core/src/session/turn.rs:155-180,225-269,460-499`; `codex-rs/core/src/session/mod.rs:3496-3537`). For a mid-turn sequence whose checkpoint replacement is `K = [P, U2]`, followed by assistant continuation `A2`, turn completion, and `rollback(1)`, ordinary forward application of the checkpoint and rollback yields prior compacted state `P`. Current reconstruction instead discards `K`, starts materialization after it, records only `A2`, and leaves `A2` untouched because `drop_last_n_user_turns` is a no-op when that truncated suffix contains no user boundary (`codex-rs/core/src/session/rollout_reconstruction.rs:370-420`; `codex-rs/core/src/context_manager/history.rs:326-346`). The model therefore loses the complete compacted prefix and can receive an orphaned continuation, directly establishing rework risk. |
| H082 | **Plausible** | Manual local, remote-v1, and remote-v2 non-token-budget compaction use `DoNotInject`, rehydrate current `additional_content.*` messages into the `CompactedItem`, and persist no WorldState baseline (`codex-rs/core/src/compact.rs:61-112,367-388`; `codex-rs/core/src/compact_remote.rs:269-303,311-350`; `codex-rs/core/src/compact_remote_v2.rs:309-352`; `codex-rs/core/src/session/additional_context.rs:9-51`). On cold resume or fork before any ordinary turn, reconstruction clears the old baseline and restores an empty store, so identical explicit republishing appends a second retained message that prompt normalization does not deduplicate (`codex-rs/core/src/session/rollout_reconstruction.rs:189-222,472-486`; `codex-rs/core/src/session/mod.rs:1460-1513`; `codex-rs/core/src/state/additional_context.rs:53-93`; `codex-rs/core/src/context_manager/history.rs:165-223`). Mid-turn and token-budget compaction persist a baseline and are unaffected; pre-turn exposure requires interruption or fork before its immediate rewrite, and app-server publication requires experimental `additionalContext` (`codex-rs/core/src/session/turn.rs:1127-1184`; `codex-rs/core/src/compact_token_budget.rs:21-89`; `codex-rs/app-server-protocol/src/protocol/v2/turn.rs:174-177,289-292`). |
| H083 | **Plausible** | The experimental v2 `turn/start` and `turn/steer` payloads accept `HashMap<String, AdditionalContextEntry>` with plain string keys, and app-server maps every entry directly into Core. Its 1,048,576-character input check counts ordinary user/tool-output text, not `additionalContext`; the public integration test confirms an enabled client can place such a fragment in model input, and `thread/compact/start` exposes the manual-compaction trigger (`codex-rs/app-server-protocol/src/protocol/v2/turn.rs:109-115,174-177,289-292`; `codex-rs/app-server/src/request_processors/turn_processor.rs:94-120,539-559,583-605`; `codex-rs/protocol/src/user_input.rs:8-9`; `codex-rs/app-server/tests/suite/v2/turn_start.rs:413-472`; `codex-rs/app-server-protocol/src/protocol/common.rs:657-660`; `codex-rs/app-server/src/request_processors/thread_processor.rs:2344-2356`). Rendering truncates only each value to 1,000 approximate tokens; both untrusted and application renderers interpolate the key into the opening and closing wrapper, with no key-length, entry-count, or aggregate limit (`codex-rs/context-fragments/src/additional_context.rs:6-18,27-38,42-56,77-101`; `codex-rs/context-fragments/src/fragment.rs:91-113`; `codex-rs/core/src/state/additional_context.rs:53-84`). For an untrusted key `a>b`, the exact rendered text is `<external_a>b>v</external_a>b>`: `matches_text` splits at the first `>`, derives key `a`, and rejects the actual `</external_a>b>` suffix. Contextual recognition is text-only, so event mapping parses the fragment as a real user message (`codex-rs/context-fragments/src/additional_context.rs:42-52,94-96`; `codex-rs/core/src/context/contextual_user_message.rs:18-43`; `codex-rs/core/src/event_mapping.rs:64-65,98-103,180-191`). On the reachable local-compaction path, that false user message enters the 20,000-token retained-user budget. Rebuilding preserves its text but rewrites its content kind to `user.text`; compaction rehydration then fails to find `additional_content.a>b` in the replacement, clones the original canonical item from old history, installs both copies, and persists that replacement (`codex-rs/core/src/compact.rs:352-379,541-570,644-720`; `codex-rs/context-fragments/src/annotated_content.rs:74-97`; `codex-rs/core/src/session/additional_context.rs:10-65`; `codex-rs/core/src/session/mod.rs:3496-3534`). A short malformed fragment therefore appears byte-identically twice on the next model request, and further live local compactions can retain another malformed copy until the 20K budget is occupied while rehydrating one current canonical copy outside that budget. |
| H084 | **Plausible** | An explicit application-context publication appends a `developer` message classified only as `additional_content.<key>` whenever that key's treatment or value fingerprint changes; replacing the snapshot with an empty map commits the clear but emits no tombstone (`codex-rs/app-server/src/request_processors/turn_processor.rs:94-118`; `codex-rs/core/src/state/additional_context.rs:48-84`; `codex-rs/context-fragments/src/additional_context.rs:59-101`). When `retain_client_developer_messages` is enabled, the turn-input path marks each such developer message with the generic one-bit `client_authored` sidecar and appends it to history; neither the sidecar nor the message classification identifies a version, current value, clear, or supersession (`codex-rs/core/src/session/turn_input.rs:640-659`; `codex-rs/core/src/session/inject.rs:70-112`; `codex-rs/history/src/lib.rs:37-51`). Remote compaction V2 retains every message satisfying that generic predicate and newest-first fills a 64,000-token retained-message budget; local new-context-window compaction independently filters the whole history by the same predicate and applies the same budget (`codex-rs/core/src/compact_remote_v2.rs:77-80,480-516,580-649`; `codex-rs/core/src/session/mod.rs:3918-3965`). The common replacement step then rehydrates only missing *current* key/kind pairs: it does not remove older matching versions, does not compare the current value fingerprint, and has no work at all after an explicit clear because the current-key set is empty (`codex-rs/core/src/session/additional_context.rs:9-40,46-68`; `codex-rs/core/src/session/mod.rs:3496-3534`). The retained messages remain ordinary model input because prompt normalization has no message-deduplication or supersession pass (`codex-rs/core/src/context_manager/history.rs:217-223,459-477`; `codex-rs/core/src/session/turn.rs:367-383`). Consequently, repeated application-context versions survive compaction as obsolete developer history, and after a clear the entire retained slice can be obsolete application context. The defect is conditional: the retention feature is under development and default-disabled (`codex-rs/features/src/lib.rs:1633-1638`). |
| H085 | **Plausible** | `WorldStateSectionContribution::new` accepts an arbitrary `serde_json::Value` comparison snapshot and documents no non-null invariant; its fragment body is likewise an unrestricted `String` (`codex-rs/ext/extension-api/src/contributors/world_state.rs:35-55,74-104,124-136`). Core normalizes extension snapshots by removing object-valued null fields and then converts a top-level `null` to `None`; `WorldState::snapshot` consequently omits that section altogether (`codex-rs/core/src/context/world_state/mod.rs:145-180,402-418,504-513`). A stable extension whose renderer emits only for `PreviousWorldStateSection::Absent` therefore emits once during full initial-context rendering, but the persisted/in-memory baseline has no entry for its ID (`codex-rs/core/src/session/mod.rs:3632-3857,3988-4015`). Before the first request, `run_turn` sends the same first step through `record_step_world_state_if_changed`; that method snapshots both the prior and rebuilt states, finds the extension absent from both snapshots, maps the missing prior entry back to `Absent`, and appends the fragment despite there being no merge patch (`codex-rs/core/src/session/turn.rs:208-250,302-378`; `codex-rs/core/src/session/mod.rs:3232-3264`; `codex-rs/core/src/context/world_state/mod.rs:421-430`). The same pre-request path runs on every follow-up inference. Thus request 1 contains the initial copy plus one pre-request copy, and request M contains M+1 copies; absent compaction, the active extension text is `(M+1)B` and cumulative transmitted text is `B * M(M+3)/2`. Each emitted message is appended unchanged, persisted as a `ResponseItem`, restored on resume, and sent by `for_prompt`, while the null section still has no durable WorldState entry (`codex-rs/core/src/context_manager/history.rs:188-223,479-540`; `codex-rs/core/src/session/mod.rs:3191-3220`; `codex-rs/core/src/session/rollout_reconstruction.rs:378-427,471-485`; `codex-rs/core/src/session/mod.rs:1460-1512`). There is no generic section-count or fragment-size cap in registration, contribution collection, rendering, or message-history processing (`codex-rs/ext/extension-api/src/registry.rs:110-112,244-247`; `codex-rs/core/src/session/world_state.rs:282-300`; `codex-rs/context-fragments/src/fragment.rs:55-106`). Auto-compaction/model context limits bound successful active requests by a context window and may periodically reset history, but they run from post-sampling usage and do not prevent the first oversized fragment or the triangular growth within each window (`codex-rs/core/src/session/context_window.rs:23-74`; `codex-rs/core/src/session/turn.rs:410-499`). The defect is conditional rather than currently exercised by an in-tree production contributor: Skills uses an object snapshot even when its body is `None`, and Git attribution uses a boolean (`codex-rs/ext/skills/src/world_state.rs:77-118`; `codex-rs/ext/git-attribution/src/world_state.rs:30-58`). |
| H086 | **Plausible** | The Apps, plugin, and environment sections snapshot only a Boolean, render nothing whenever the current value is false, and emit their complete deterministic developer fragment for `Known(false) -> true` (`codex-rs/core/src/context/world_state/apps_instructions.rs:18-51`; `codex-rs/core/src/context/world_state/plugins_instructions.rs:18-51`; `codex-rs/core/src/context/world_state/environments_instructions.rs:18-50`). The silent disable still advances `ContextManager`'s baseline and is persisted as a `WorldStateItem`, including when no model-visible item was generated (`codex-rs/core/src/context_manager/history.rs:133-149`; `codex-rs/core/src/session/mod.rs:3988-4061`). The old guidance therefore remains in append-only response history. On re-enable, retained-fragment detection does not suppress reinjection: finding the old fragment preserves the exact persisted `Known(false)` state, which makes the section emit the same full fragment again (`codex-rs/core/src/context/world_state/mod.rs:433-482`). Fragment merging only groups new content, history recording pushes it, normalization has no message deduplication, and WebSocket reuse accepts only strict history extension, so neither content-kind annotations nor transport incrementality replace an older same-kind fragment (`codex-rs/core/src/context_manager/updates.rs:27-60`; `codex-rs/core/src/context_manager/history.rs:188-203,459-476`; `codex-rs/core/src/client.rs:1310-1352`). Rollout reconstruction independently replays response items into history and WorldState records into the baseline, preserving the old text plus the disabled snapshot across resume (`codex-rs/core/src/session/rollout_reconstruction.rs:370-510`; `codex-rs/core/src/session/mod.rs:1465-1512`). Compaction rebuilds/replaces the history and canonical baseline, so it bounds accumulation rather than disproving the pre-compaction duplicate (`codex-rs/core/src/compact.rs:344-389,644-732`; `codex-rs/core/src/compact_remote.rs:332-389`; `codex-rs/core/src/session/mod.rs:3496-3538`). |
| H087 | **Plausible** | Legacy remote compaction is reachable for a provider that advertises remote compaction when `RemoteCompactionV2` is disabled and the separate `TokenBudget` feature is not enabled: both manual and inline dispatch then select `compact_remote` rather than v2 or local compaction (`codex-rs/core/src/tasks/compact.rs:32-76`; `codex-rs/core/src/session/turn.rs:1194-1272`). That path builds a model-specific request, including reasoning settings, and calls the unary `/responses/compact` endpoint, but its result type is only `Vec<ResponseItem>`; `CompactHistoryResponse` deserializes only `output` and exposes no `TokenUsage` or provider budget units (`codex-rs/core/src/compact_remote_request.rs:24-103`; `codex-rs/core/src/client.rs:569-677`; `codex-rs/codex-api/src/endpoint/compact.rs:39-88`). After a successful call, v1 installs the returned history and invokes `recompute_token_usage`, but never invokes `record_rollout_budget_usage`; the same is true when the first model-specific compaction fails and the current-model fallback succeeds (`codex-rs/core/src/compact_remote.rs:192-308`; `codex-rs/core/src/compact_model_fallback.rs:8-20`). By contrast, ordinary completed sampling responses flow through `record_token_usage_info` into the shared rollout ledger, and both local and remote-v2 completed compactions charge that ledger; exhausting it returns `SessionBudgetExceeded` and prevents a compaction retry (`codex-rs/core/src/session/turn.rs:2582-2615`; `codex-rs/core/src/session/mod.rs:4076-4109`; `codex-rs/core/src/session/rollout_budget.rs:25-35`; `codex-rs/core/src/compact.rs:735-791`; `codex-rs/core/src/compact_remote_v2.rs:294-307`; `codex-rs/core/tests/suite/rollout_budget.rs:334-404`). Therefore every successful v1 compaction model call is omitted from weighted usage, reminders, and the post-response hard-stop check; an inline pre-turn or mid-turn compaction can continue into another sampling request where the charged paths would have stopped. The defect is conditional rather than default-wide: `RolloutBudget` is disabled by default, `RemoteCompactionV2` is enabled by default, and enabling `TokenBudget` bypasses the legacy remote path (`codex-rs/features/src/lib.rs:1490-1500,1622-1626`; `codex-rs/core/src/config/config_tests.rs:807-839`). |
| H088 | **Plausible** | `CopilotAuthProvider` defines a substantive WebSocket transform: it parses the serialized frame, applies `normalize_websocket`, adds Copilot per-frame identity, and re-encodes the result; normalization removes unsupported request fields and web-search tools and drops every input item before the latest `compaction` (`codex-rs/model-provider/src/copilot/auth_provider.rs:139-157`; `codex-rs/model-provider/src/copilot/payload.rs:3-44`). Production never invokes that hook. `ResponsesWebsocketClient::connect` uses the auth provider only to mutate upgrade headers and then builds a `ResponsesWebsocketConnection` that does not retain auth; `stream_request` directly serializes the generic `ResponsesWsRequest`, and `send_websocket_request` transmits that string unchanged (`codex-rs/codex-api/src/endpoint/responses_websocket.rs:185-223,344-425,235-273,904-937`). No earlier generic step is equivalent: request construction fills `tool_choice`, `stream`, `prompt_cache_key`, and `client_metadata`, the WebSocket conversion copies those fields, and history prompt normalization only repairs tool pairs/media rather than cutting around compaction (`codex-rs/core/src/client.rs:888-1002,1818-1879`; `codex-rs/codex-api/src/common.rs:302-390`; `codex-rs/core/src/context_manager/history.rs:207-223,459-477`). A model-visible witness is reachable when a remotely compacted thread is resumed with an explicit Copilot provider override: remote compaction v2 may retain up to 64,000 tokens of user/developer/system messages before its final `Compaction` item, persistence installs that replacement history, and `thread/resume` accepts `modelProvider` into the new config (`codex-rs/core/src/compact_remote_v2.rs:74-78,489-509,534-558`; `codex-rs/core/src/session/mod.rs:3496-3535`; `codex-rs/app-server-protocol/src/protocol/v2/thread.rs:307-350`; `codex-rs/app-server/src/request_processors/thread_processor.rs:3612-3634,3714-3799`). The intended hook would send only the latest compaction and suffix; the current first/full Copilot WebSocket request sends the retained prefix too. Separately, the Copilot frame test treats injected `headers`, `agent_task_id`, and `initiator` as the Substrate envelope, so a backend that requires that contract can reject the raw frame and fail the request (`codex-rs/model-provider/src/copilot/auth_provider_tests.rs:174-225`). |
| H089 | **Plausible** | The checked-in `gpt-5.6-sol`, `gpt-5.6-terra`, and `gpt-5.6-luna` entries all use Responses Lite and contain the same literal instruction template with no instruction variables (`codex-rs/models-manager/models.json:4-73,135-204,261-326`; `codex-rs/protocol/src/openai_models.rs:534-550`). A read-only decode measured each template at exactly 17,766 UTF-8 bytes with SHA-256 `cbefa6b0bede0e332d957fca70ccacf9f12f4c0ecdf81b819e5cbe1a3b16e265`. The session chooses base instructions once from configuration, resumed history, or its startup model, and every sampling request continues to fetch that fixed session value (`codex-rs/core/src/session/mod.rs:671-686`; `codex-rs/core/src/session/turn.rs:1357-1404`). After a prior turn, World State derives the prior model from `previous_turn_settings`, while `ModelInstructionsState` persists only the model slug and renders the destination's complete instructions whenever the slug differs; it never compares instruction content or stores the available `WorldStateHash` fingerprint (`codex-rs/core/src/session/world_state.rs:41-86`; `codex-rs/core/src/context/world_state/model.rs:24-59`; `codex-rs/core/src/context/world_state/mod.rs:266-280`). The resulting standalone `<model_switch>` developer fragment embeds the full template, is appended without message deduplication, and is persisted as ordinary history (`codex-rs/core/src/context/model_switch_instructions.rs:17-38`; `codex-rs/context-fragments/src/fragment.rs:55-118`; `codex-rs/core/src/context_manager/history.rs:126-151,160-205`; `codex-rs/core/src/session/mod.rs:3174-3220,3990-4046`). Responses Lite then prepends the same fixed base text as another developer item and sends both in `input`; changing `model` also prevents WebSocket `previous_response_id` delta reuse, so a switch sends the full request rather than suppressing the duplicate (`codex-rs/core/src/context/base_instructions.rs:1-30`; `codex-rs/core/src/client.rs:329-384,894-931,970-989,1320-1352,1817-1859`). Thus a Sol→Terra→Luna sequence with otherwise unchanged settings adds one byte-identical 17,766-byte instruction body per transition, and retained copies accumulate until a history rewrite. The clean-session first-turn-override subclaim is overstated: when the startup base already equals the override model's instructions, the provenance fallback deliberately withholds a previous model, so no switch fragment is rendered (`codex-rs/core/src/session/world_state.rs:48-64`; `codex-rs/core/src/context/world_state/model.rs:43-59`). |
| H090 | **Plausible** | Durable-history recording stamps metadata-capable items with a turn ID and stamps harness-authored user/developer/agent/tool-output items with creation time; default-enabled content classifications add `content_item_kinds` to ordinary messages (`codex-rs/core/src/session/mod.rs:3057-3169`; `codex-rs/protocol/src/models.rs:919-955,1288-1356`; `codex-rs/features/src/lib.rs:934-938`). Full-history recomputation sums `estimate_item_token_count` over the stored items, and ordinary items are estimated from their complete compact JSON serialization at four bytes per token, so that metadata is charged (`codex-rs/core/src/context_manager/history.rs:269-283,696-769`; `codex-rs/utils/string/src/truncate.rs:4,71-83`). Request construction later clears the entire passthrough object whenever `provider.info().is_openai()` is false, and both HTTP and WebSocket encoding occur only after that mutation; checked-in tests prove direct OpenAI preserves the metadata while a differently named Responses provider omits it but retains generated item IDs (`codex-rs/core/src/client.rs:884-945,993-1001,1628-1647,1833-1855`; `codex-rs/codex-api/src/endpoint/responses.rs:102-116`; `codex-rs/codex-api/src/endpoint/responses_websocket.rs:935-938`; `codex-rs/core/tests/suite/client.rs:202-356`). A usage-null completion, every compaction replacement, rollback, and token-budget context replacement recompute from this unnormalized history; `context_window_token_status` then uses the resulting `last_token_usage.total_tokens` to decide pre-turn or mid-turn compaction and report remaining capacity (`codex-rs/core/src/session/turn.rs:405-480,2585-2608,1027-1057`; `codex-rs/core/src/compact.rs:360-390`; `codex-rs/core/src/compact_remote.rs:288-304`; `codex-rs/core/src/compact_remote_v2.rs:340-354`; `codex-rs/core/src/session/handlers.rs:334-344`; `codex-rs/core/src/session/mod.rs:3957-3965,4112-4148`; `codex-rs/core/src/session/context_window.rs:23-90`). A representative default user item gains about 160-163 serialized bytes (UUID turn ID, fractional creation time, and `["user.text"]`), or about 40-41 heuristic tokens; 500 such items therefore add about 20K tokens that are absent from the transmitted non-OpenAI request. For a 272,000-token model, the standard auto-compaction threshold is 244,800, so a history whose estimate after metadata clearing is 224,800 crosses the threshold solely because of this 20K charge (`codex-rs/protocol/src/openai_models.rs:503-519,1853-1869`; `codex-rs/models-manager/models.json:20-35`). Provider-reported usage on a later usage-bearing completion replaces the active last-usage basis and bounds the discrepancy, but it cannot prevent a pre-turn compaction that runs before that next completion. |

| H091 | **Plausible** | Both Guardian request builders preserve every heading and transcript entry as a separate text element: synchronous Guardian repeatedly pushes `UserInput::Text`, whose protocol conversion emits one `ContentItem::InputText` per element, while Guardian V2 builds a `Vec<String>` and maps each string one-for-one into the classifier user message (`codex-rs/core/src/guardian/prompt.rs:97-103,220-326`; `codex-rs/protocol/src/models.rs:1948-2024`; `codex-rs/ext/guardian-v2/src/async_scorer/extension.rs:557-600`; `codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs:491-510`). The provider-bound path has no adjacent-text merge: core request preparation only adjusts IDs/content-kind metadata, and WebSocket transport directly `serde_json::to_string`s the typed request (`codex-rs/core/src/client.rs:993-1001,1837-1859`; `codex-rs/codex-api/src/endpoint/responses_websocket.rs:235-269,935-937`). Checked-in request evidence consequently contains a 17-part synchronous Guardian user message and an 11-part V2 classifier user message (`codex-rs/core/src/guardian/snapshots/codex_core__guardian__tests__guardian_review_request_layout.snap:7-29`; `codex-rs/ext/guardian-v2/src/async_scorer/extension_tests.rs:1941-1962`). Compact JSON adds exactly 32 bytes—`,{"type":"input_text","text":""}`—for each boundary after the first, so 40 parts add `39 * 32 = 1,248` bytes; the repository's four-bytes-per-token ceiling heuristic maps that exact delta to 312 tokens (`codex-rs/protocol/src/models.rs:851-860`; `codex-rs/core/src/context_manager/history.rs:696-703,727-755`; `codex-rs/utils/string/src/truncate.rs:71-83`). Synchronous turn submission stores the multipart message, prompt normalization does not merge it, and the follow-up snapshot resends the original 16-part prompt before adding the new 14-part delta, establishing cumulative logical-history cost (`codex-rs/core/src/session/mod.rs:3139-3175,4219-4231`; `codex-rs/core/src/context_manager/history.rs:207-223,460-476`; `codex-rs/core/src/guardian/snapshots/codex_core__guardian__tests__guardian_followup_review_request_layout.snap:30-75`). The impact is bounded but material: 40 is only the default non-user-entry cap, user entries are separately limited by text-token budgets, and V2 allows transcript budgets up to 100,000 tokens while requiring only that `max_recent_non_user_entries` be positive, so envelope bytes are outside the selection budget and total parts can substantially exceed 40 (`codex-rs/core/src/guardian/mod.rs:69-75`; `codex-rs/core/src/guardian/prompt.rs:457-493`; `codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs:20-25,365-400`; `codex-rs/features/src/feature_configs.rs:85-108,190-224`). Exact remote tokenizer/billing treatment remains unmeasured, but the uncoalesced request structure, raw serialized inflation, local estimator inflation, and synchronous retention are deterministic. |
| H092 | **Plausible** | A synchronous full Guardian review carries the four-property output contract twice in one request. The Guardian base instructions append a pseudo-schema that repeats `risk_level`, `user_authorization`, `outcome`, `rationale`, every enum value, and each property's string type; the turn also installs `guardian_output_schema()`, which the shared request builder serializes under `text.format` as `type: "json_schema"` (`codex-rs/core/src/guardian/prompt.rs:775-843`; `codex-rs/core/src/guardian/review_session.rs:1165-1194`; `codex-rs/core/src/session/turn.rs:1331-1344`; `codex-rs/core/src/client.rs:965-988`; `codex-rs/codex-api/src/common.rs:220-246,423-443`). The raw-request test confirms `strict: false` and the exact schema in `/text/format/schema` (`codex-rs/core/src/guardian/tests.rs:1995-2204`). The repeated prompt block is 216 bytes, or 54 tokens by the repository's four-bytes-per-token heuristic, while the compact wire schema is 319 bytes/80 heuristic tokens, so the overlapping contract is credibly several dozen input tokens per uncached request. The prose copy is not wholly redundant: it also directs read-only investigation, permits bare `{"outcome":"allow"}` for low risk, and asks for the fuller payload otherwise, behavior not enforced by a non-strict schema requiring only `outcome`. Routing to `/guardian` instead of `/responses`, and Responses Lite moving base instructions into a developer message, do not remove either copy. The lightweight Guardian V2 `/guardian-classifier` request is separate and explicitly has `text: None`, so this finding is limited to synchronous full Guardian reviews. |
| H093 | **Plausible** | Successful skill and plugin activations are selected from the current turn's explicit `UserInput`, deduplicated only inside fresh per-call sets, then concatenated into `injection_items` without any count, byte, or token-sum check. Extension-selected and Agent Plugin skill bodies are individually truncated to 8,000 bytes (plus wrappers), and each explicit plugin hint is individually truncated to 4 KiB, but every distinct bounded item is recorded separately in ordinary history; selecting many in one turn therefore contributes their full aggregate to the first sampling request, while reselecting one on successive turns leaves one additional copy per turn until compaction or rollback (`codex-rs/skills/src/selection.rs:42-105,129-194`; `codex-rs/ext/skills/src/selection.rs:21-101`; `codex-rs/core/src/plugins/mentions.rs:49-92`; `codex-rs/ext/skills/src/extension.rs:400-500`; `codex-rs/ext/skills/src/render.rs:17-29,1176-1182`; `codex-rs/core/src/plugins/render.rs:6-87`; `codex-rs/core/src/session/turn.rs:773-914`). The bounded skills-catalog budget is separate: catalog rendering is limited to 2% of the context window or at most 10,000 configured tokens and may omit entries, but selection runs against the full catalog before selected prompt bodies are rendered, so catalog omission does not cap activation count or bytes (`codex-rs/ext/skills/src/render.rs:127-153,650-820`; `codex-rs/ext/skills/src/extension.rs:373-500`). History processing clones `Message` items unchanged, and `for_prompt` only repairs call/output pairs and strips unsupported media; each injection is also persisted as a rollout `ResponseItem` and replayed on resume (`codex-rs/core/src/context_manager/history.rs:165-223,479-538`; `codex-rs/core/src/session/mod.rs:3175-3220,3867-3873`; `codex-rs/core/src/session/rollout_reconstruction.rs:320-420`; `codex-rs/core/src/session/mod.rs:1460-1512`). A fresh `ModelClientSession` is created per turn, so the first request of each turn uses the full surviving history; absent intervening compaction, a B-byte activation selected on T turns occupies T*B bytes in turn T's logical prompt and contributes B*T*(T+1)/2 bytes across those first requests. WebSocket deltas can avoid retransmitting the old prefix only for continuations inside one turn, not for the next turn's fresh session (`codex-rs/core/src/session/turn.rs:155-171,371-390`; `codex-rs/core/src/client.rs:280-309,1311-1353`). Direct host-snapshot skills are worse than the bounded witness: Core truncates only roots identified as Agent Plugins; repo, user/admin, bundled-system, and legacy recursive-plugin skills retain the full file text, with local file reads capped only at 512 MiB. The checked-in `imagegen` system skill is 19,516 bytes, about 4,879 tokens under the repository's four-bytes-per-token estimate, and one integration test explicitly proves a host prompt beyond 8 KiB remains intact (`codex-rs/ext/skills/src/host_prompt.rs:69-103`; `codex-rs/ext/skills/src/loader/host.rs:54-143,147-192,400-431`; `codex-rs/core-plugins/src/loader.rs:887-894`; `codex-rs/exec-server/src/local_file_system.rs:45-52,586-604`; `codex-rs/skills/src/assets/samples/imagegen/SKILL.md:1-315`; `codex-rs/utils/string/src/truncate.rs:4,71-73`; `codex-rs/core/tests/suite/skills_extension.rs:2664-2706`). Compaction and the model context window are coarse eventual bounds, not an activation aggregate bound: local, remote-v1/v2, and token-budget replacement histories remove these raw skill/plugin messages, but pre-turn compaction intentionally runs before current context, user input, and activations are recorded, so a bounded multi-activation batch can materially enlarge or overflow its very first request; accumulated copies also inflate any model-backed compaction request that precedes their removal (`codex-rs/core/src/session/turn.rs:155-282`; `codex-rs/core/src/compact.rs:249-310,533-570`; `codex-rs/core/src/compact_remote.rs:354-379`; `codex-rs/core/src/compact_remote_v2.rs:480-560`; `codex-rs/core/src/session/mod.rs:3918-3965`; `codex-rs/core/src/session/context_window.rs:23-86`). |
| H094 | **Plausible** | Each skills world-state section snapshots its complete rendered `body` plus availability/config flags. Exact equality suppresses unchanged output, but a model-visible difference returns the complete current body rather than an entry-level delta; the focused test changes a one-skill host snapshot by adding one short skill and confirms the section renders again (`codex-rs/ext/skills/src/world_state.rs:68-118`; `codex-rs/ext/skills/src/world_state_catalogs.rs:251-329`; `codex-rs/ext/skills/tests/skills_extension.rs:234-352`). The resulting developer message is appended to `ContextManager`; message normalization only repairs tool-call pairs and strips unsupported media, and the next sampling input is the full normalized history, so the prior complete catalog and its replacement are simultaneously model-visible (`codex-rs/core/src/session/mod.rs:3232-3262`; `codex-rs/core/src/context_manager/updates.rs:20-60`; `codex-rs/core/src/context_manager/history.rs:188-224,459-536`; `codex-rs/core/src/session/turn.rs:367-377`). The repeated bytes are meaningful even for a tiny edit: before roots or entries, the three catalog forms already contribute 270-458 bytes of fixed body, and larger replacements can consume the shared skill-line budget of 2% of the context window (5,440 at 272K), a configured maximum of 10,000 approximate tokens, or the 8,000-character fallback, with common framing and most optional usage guidance outside that skill-line allocation (alias-specific overhead is deducted when aliases are chosen); ordinary message history does not truncate these developer messages (`codex-rs/ext/skills/src/catalog_prompt.rs:1-15,81-105`; `codex-rs/ext/skills/src/render.rs:17-24,127-170,317-381,540-721`; `codex-rs/core/src/context_manager/history.rs:478-536`). Apps and plugins likewise emit no removal on `true -> false`, then emit the entire guidance again on `false -> true`; the checked-in repeated bodies are 646 and 1,014 bytes (about 415 tokens at the renderer's four-bytes-per-token approximation), supporting the claimed roughly 400-token cycle (`codex-rs/core/src/context/world_state/apps_instructions.rs:38-49`; `codex-rs/core/src/context/world_state/plugins_instructions.rs:38-49`; `codex-rs/core/src/context/world_state/snapshots/codex_core__context__world_state__apps_instructions__tests__snapshots.snap:11-34`; `codex-rs/core/src/context/world_state/snapshots/codex_core__context__world_state__plugins_instructions__tests__snapshots.snap:11-42`). Compaction bounds the accumulation: local compaction rebuilds history and reinjects current initial/world-state context, while remote compaction filters developer messages before the same reinjection, so stale catalog versions do not survive a completed compaction (`codex-rs/core/src/compact.rs:91-111,350-386`; `codex-rs/core/src/compact_remote.rs:333-377`; `codex-rs/core/src/compact_tests.rs:343-407`). |
| H095 | **Plausible** | Endpoint mode does not dump the global marketplace, but it does copy every eligible suggestion retained from the unqueried `/ps/plugins/suggested/codex` endpoint into model-visible `<recommended_plugins>` context, subject to a 50-entry cap. The endpoint adapter validates/deduplicates names, caps plugin and display names at 64 characters, and the manager removes locally or remotely installed plugins, explicitly disabled suggestions, and plugins for the `codex-tui` app-server client (`codex-rs/core-plugins/src/remote.rs:1017-1081`; `codex-rs/core-plugins/src/manager.rs:1767-1835`; `codex-rs/tools/src/tool_discovery.rs:134-151`). Every remaining name and config ID is rendered; no user-request predicate gates this initial-context injection (`codex-rs/core/src/context/recommended_plugins_instructions.rs:5-53`; `codex-rs/core/src/session/mod.rs:3659-3689,3810-3855`). That item is recorded when the reference context is absent, and each sampling step builds its input from the retained history, so one bounded copy remains model-visible on every request until context replacement; compaction/new-window rebuilding can install a fresh copy (`codex-rs/core/src/session/mod.rs:3992-4048`; `codex-rs/core/src/session/turn.rs:367-377`; `codex-rs/core/src/context_manager/history.rs:188-223`; `codex-rs/core/src/session/mod.rs:3935-3965`). The checked-in cap test proves 50 entries are accepted (`codex-rs/core-plugins/src/remote_tests.rs:555-598`). With 64-character display names and 64-character ASCII plugin-name segments, 50 rendered lines are roughly 7.8 KB before framing (about 2K repository-heuristic tokens), while four-byte display characters can approach the claimed 4K range (`codex-rs/plugin/src/plugin_id.rs:15-19,50-78`; `codex-rs/utils/string/src/truncate.rs:71-77`). Legacy discovery is more filtered than H095 suggests: plugins must be explicitly configured, belong to a fixed 30-ID fallback allowlist, or be a remote plugin matching a loaded app; installed, policy-unavailable, admin-disabled, and user-disabled entries are removed. Connectors are restricted to configured or loaded-plugin connector IDs and accessible connectors are removed, so unbounded directory pagination does not itself expose the whole connector directory (`codex-rs/core-plugins/src/discoverable.rs:12-42,69-204`; `codex-rs/core/src/connectors.rs:84-115,394-461`; `codex-rs/connectors/src/filter.rs:5-25`; `codex-rs/connectors/src/lib.rs:259-289`). Even so, after those filters the queryless legacy tool serializes every survivor; only descriptions are capped at 240 characters, while candidate count and MCP-server/app-ID arrays have no local aggregate ceiling (`codex-rs/core/src/tools/handlers/list_available_plugins_to_install.rs:14-53,72-100`; `codex-rs/tools/src/tool_discovery.rs:155-210`). Generic function-output truncation is the final bound and removes the middle of oversized text, so a sufficiently large configured/installed-plugin candidate set can consume the full output allowance and omit a requested middle entry (`codex-rs/core/src/context_manager/history.rs:613-633`; `codex-rs/utils/output-truncation/src/lib.rs:17-34`; `codex-rs/utils/string/src/truncate.rs:1-68`). Thus the broad “whole global catalogs” wording and connector-pagination rationale are overstated, but the bounded per-request endpoint tax and the uncapped filtered legacy catalog are real reachable waste paths. |
| H096 | **Plausible** | `skills.max_context_tokens` is documented as the maximum tokens used by the available-skills catalog, but the shared renderer applies that limit primarily to the per-skill entry lines: `allocate_skill_lines` compares the sum of each rendered entry to the limit, then `render_combined_available_skills` splits those allocations back into executor, orchestrator, and host outputs (`codex-rs/config/src/skills_config.rs:36-43`; `codex-rs/ext/skills/src/render.rs:127-160,325-365,540-658,710-788`). Aliased rendering additionally reserves only the alias form's incremental cost over an empty unaliased body; that calculation includes root-table/intro differences and the resource-alias line, but excludes the ordinary body baseline, package-read paragraph, markers, usage heading, and complete usage body (`codex-rs/ext/skills/src/render.rs:792-836,871-895,1087-1108`). After allocation, each nonempty authority output independently appends `### How to use skills` and a complete usage block when the model enables it, while `render_available_skills_body` independently emits `## Skills`, its generic/source-specific introduction, root heading, package-read guidance, and `### Available skills` (`codex-rs/ext/skills/src/fragments.rs:18-31`; `codex-rs/ext/skills/src/catalog_prompt.rs:3-40,81-104`). The three outputs become separate World State sections and separate marked `<skills_instructions>` content items; initial-context aggregation puts them in one developer message but preserves every rendered item rather than deduplicating their text (`codex-rs/ext/skills/src/world_state_catalogs.rs:251-298`; `codex-rs/ext/skills/src/world_state.rs:86-118`; `codex-rs/context-fragments/src/fragment.rs:91-105`; `codex-rs/core/src/session/mod.rs:3783-3819`; `codex-rs/core/src/context_manager/updates.rs:12-29`). A read-only reconstruction from the checked-in strings and the repository's `ceil(UTF-8 bytes / 4)` estimator measured three aliased authority wrappers at 10,641 fixed bytes, about 2,661 tokens, excluding root and entry lines; the unaliased equivalent is 11,405 bytes, about 2,852 tokens. A one-wrapper structural reconstruction using the existing broad locator wording is about 982 fixed tokens in the aliased case, exposing about 6,714 bytes/1,679 tokens as potentially shareable; the unaliased delta is about 7,494 bytes/1,874 tokens. Exact preservation of source-specific host phrasing would consume part of that delta, so the claimed 1.5K-1.8K is an optimization estimate rather than a byte-identical duplicate count, but it is consistent with the measured structure. Thus a line allocation near the configured 10,000-token ceiling can produce roughly 12,700 model-visible skill tokens under the same estimator. Current `gpt-5.6-sol`, Terra, and Luna disable the optional usage block, but three aliased authorities still add about 1,667 fixed bytes/417 tokens outside the charged entry budget, with about 822 bytes/206 tokens removable by a single shared wrapper (`codex-rs/models-manager/models.json:4-23,135-154,261-280`). Authority and safety semantics are real requirements: entry lines explicitly distinguish `file`, `executor package`, `orchestrator package`, and `custom resource`; package locators must go through `skills.read`; and the usage text enforces full reads, non-delegation of instruction interpretation, and safe fallback (`codex-rs/ext/skills/src/render.rs:200-257`; `codex-rs/ext/skills/src/catalog_prompt.rs:3-40`). Those rules need to remain model-visible, but the same generic trigger, fallback, coordination, context-hygiene, and safety rules do not need to be repeated once per authority, and independent World State identities do not justify excluding their rendered cost from the advertised cap. |
| H097 | **Plausible** | Phase 1 has no application-level size bound on either corpus field: its strict output schema declares `raw_memory` and `rollout_summary` only as strings, the extraction prompt explicitly says the summary has “no strict size limit” and asks for detailed raw memory, the response loop appends all streamed text, and SQLite persists both complete values as unconstrained `TEXT` (`codex-rs/memories/write/src/phase1.rs:43-54,135-146,300-318`; `codex-rs/memories/write/templates/memories/stage_one_system.md:222-249,401-557`; `codex-rs/memories/write/src/runtime.rs:253-305`; `codex-rs/state/migrations/0006_memories.sql:1-8`; `codex-rs/state/src/runtime/memories.rs:850-929`). Phase 2 applies only a row-count/age/ranking selection—256 rows by default and 4,096 at the configured maximum—and then copies every selected `raw_memory` into one file and every complete `rollout_summary` into a per-thread file, without summing bytes or tokens (`codex-rs/config/src/types.rs:47-56,301-389`; `codex-rs/state/src/runtime/memories.rs:430-542`; `codex-rs/memories/write/src/storage.rs:44-74,110-142`; `codex-rs/memories/write/src/phase2.rs:55-128,214-222`). A concrete within-default witness needs no extreme item: 256 rows containing only 4 KiB in each field expose about 2 MiB, or 524,288 tokens under the repository's four-bytes-per-token heuristic, before metadata and instructions. The default consolidation model is Terra with a 272,000-token context and a 244,800-token automatic-compaction threshold; INIT explicitly requires a complete chunked scan of `raw_memories.md` followed by careful rollout-summary reading (`codex-rs/model-provider/src/provider.rs:129-169`; `codex-rs/models-manager/models.json:135-163`; `codex-rs/protocol/src/openai_models.rs:511-521`; `codex-rs/utils/string/src/truncate.rs:1-3,71-78`; `codex-rs/memories/write/templates/memories/consolidation.md:119-161,768-837`). The first provider request contains paths and instructions rather than the corpus inline, and normal tool results default to 10,000 tokens, but those are per-request protections: the agent can issue many reads and the normal turn loop compacts and continues when more work remains, so they do not cap total corpus traversal (`codex-rs/memories/write/src/phase2.rs:140-153,372-378`; `codex-rs/memories/write/src/runtime.rs:322-358`; `codex-rs/core/src/unified_exec/mod.rs:74-76,216-218`; `codex-rs/core/src/session/turn.rs:413-499`). The mandatory workspace-diff source alone permits 4 MiB (about 1,048,576 heuristic tokens), and optional external-memory import recursively discovers and copies every selected project's Markdown bytes with no count or byte budget (`codex-rs/memories/write/src/lib.rs:110-113`; `codex-rs/memories/write/src/workspace.rs:132-170`; `codex-rs/external-agent-migration/src/memory.rs:17-69,106-147`; `codex-rs/external-agent-migration/src/memory_import.rs:53-169,257-309`). Thus active requests are bounded in the normal path, but the selected source corpus and cumulative model/tool/compaction work are not provider-budgeted and can materially exceed even one full context window. |
| H098 | **Plausible** | The lifecycle mismatch is real for ordinary app-server `turn/start` requests once memories are enabled. The memory documentation says the pipeline is triggered when a root **session** starts, but `turn_start_inner` recomputes `turn_has_input` for every request and, after every nonempty user-input submission that Core reports as `Started`, calls `start_memories_startup_task`; there is no once-per-thread/session marker in that path (`codex-rs/memories/README.md:29-38`; `codex-rs/app-server/src/request_processors/turn_processor.rs:514-584,642-685`). The startup function only applies eligibility gates and then spawns a fresh task that prepares the layout, prunes, checks rate limits, runs phase 1, and runs phase 2 (`codex-rs/memories/write/src/start.rs:20-80`). Phase 1 claims up to `memories.max_rollouts_per_startup` eligible stale rollouts, and every successful claim reaches one `stream_stage_one_prompt` model request; an empty claim set returns before sampling (`codex-rs/memories/write/src/phase1.rs:65-95,149-220,227-317`). State watermarks and leases prevent reprocessing the same unchanged rollout, but they do not make later startup passes no-ops while distinct backlog remains: the startup selector skips completed rows and continues to other candidates, and the focused state test completes one full 64-row batch then proves a second startup pass claims another 64 (`codex-rs/state/src/runtime/memories.rs:88-128,148-284,671-842,2559-2646`). With the feature enabled, effective defaults are two rollouts per pass and the provider default extraction model is `gpt-5.6-luna`; configuration permits up to 128, so sequential qualifying turns can cause two Luna invocations apiece (or the configured model/count) while at least that many eligible jobs remain (`codex-rs/config/src/types.rs:47-56,322-353,383-399`; `codex-rs/model-provider/src/provider.rs:129-169`). |
| H099 | **Plausible** | The claimed sizes reproduce exactly from the checked-in LF content with the repository's `ceil(UTF-8 bytes / 4)` heuristic: `stage_one_system.md` is 30,449 bytes / about 7,613 tokens, `consolidation.md` is 51,237 / about 12,810, and `read_path.md` is 6,593 / about 1,649 (`codex-rs/utils/string/src/truncate.rs:1-4,71-78`). Phase 1 installs the first template as base instructions for every claimed rollout and creates a fresh model client/session for each one; its static input framing adds another 344 LF bytes, making about 7,699 fixed tokens before rollout path, cwd, or contents. At the configurable 128-rollout maximum that is at least about 985,472 fixed tokens, so the reported 974K system-template-only figure is accurate and conservative (`codex-rs/memories/write/src/phase1.rs:189-220,282-319`; `codex-rs/memories/write/src/runtime.rs:235-319`; `codex-rs/config/src/types.rs:47-56,368-399`). Phase 2 submits the consolidation render as one user item; even after removing every template placeholder, its LF static text is about 12,749 tokens, before paths and extension blocks, directly exceeding the repository's 10K-per-item rule. Under the normal Terra model it also accompanies about 4,442 tokens of base instructions, for an initial fixed lower bound near 17.2K before tool schemas, inherited developer context, or framing (`codex-rs/memories/write/src/prompts.rs:42-80`; `codex-rs/memories/write/src/phase2.rs:312-378`; `codex-rs/core/src/session/mod.rs:671-686`; `codex-rs/models-manager/models.json:135-204`; `AGENTS.md:211-218`). The read-path item contributes about 1.6K fixed tokens plus a summary capped at 2,500, so fixed guidance remains roughly 39% even at the summary cap and dominates small summaries (`codex-rs/ext/memories/src/lib.rs:16`; `codex-rs/ext/memories/src/prompts.rs:26-66`; `codex-rs/ext/memories/src/extension.rs:41-77`). The bulk is not irreducible: Phase 1 repeats common outcome, evidence, attribution, and preference rules across its summary and raw-memory deliverables despite a strict outer JSON schema, while Phase 2 sends both INIT and incremental workflows plus repeated MEMORY, provenance, ordering, preference, and summary rules on every run. Those common rules can be factored, and Phase 2 can select or load only the applicable mode/reference sections from the workspace it already reads (`codex-rs/memories/write/src/phase1.rs:135-146`; `codex-rs/memories/write/templates/memories/stage_one_system.md:149-217,218-555`; `codex-rs/memories/write/templates/memories/consolidation.md:200-450,453-690,759-880`). |
| H100 | **Plausible** | The claim holds for the aggregate lifetime of one enabled consolidation job, not for every individual request or tool call. Phase 2 starts an internal `MemoryConsolidation` thread and hands it to a detached monitor; that monitor has no elapsed-time or step counter and renews the one-hour job lease every 90 seconds until the agent reaches a final status, the session terminates, ownership is lost, or heartbeat persistence fails (`codex-rs/memories/write/src/start.rs:54-81`; `codex-rs/memories/write/src/phase2.rs:382-489,492-555`; `codex-rs/memories/write/src/lib.rs:103-107`; `codex-rs/memories/write/src/runtime.rs:322-378`). Inside the thread, the turn engine is an unrestricted loop: each valid tool call sets `needs_follow_up`, and reaching the active context threshold while follow-up is needed runs compaction and then continues rather than terminating (`codex-rs/core/src/session/turn.rs:302-563`; `codex-rs/core/src/stream_events_utils.rs:289-356`). A focused integration test already drives one task through three successful `exec_command` calls and three successful auto-compactions before a final response, establishing that rollover is repeatable within one turn rather than a one-shot guard (`codex-rs/core/tests/suite/compact.rs:1102-1320`). Replacing that final response with another finite valid tool response can repeat the same bounded model -> tool -> compaction cycle arbitrarily many times; successful heartbeats preserve ownership throughout. Terra's context window, backend output termination, tool-output truncation, idle timeouts, and per-request retry counts limit one window/call/attempt, while the only cumulative token control, `rollout_budget`, is optional and default-disabled (`codex-rs/models-manager/models.json:135-164`; `codex-rs/protocol/src/openai_models.rs:503-521`; `codex-rs/core/src/session/context_window.rs:23-91`; `codex-rs/core/src/config/mod.rs:1035-1038`; `codex-rs/features/src/lib.rs:1488-1499`). Therefore one still-running job has no default wall-clock, model-step, tool-call, compaction-count, or cumulative-token ceiling and can generate unbounded aggregate Terra work under continuing valid tool behavior. |

#### H101-H117

| Hypothesis | Verdict | Justification |
| --- | --- | --- |
| H101 | **Plausible** | Startup claims up to configured `max_rollouts_per_startup` (1–128, default 2) under one 3,600-second lease before a per-startup `buffer_unordered(8)`, so a non-default value above eight can leave already-claimed jobs only in the local queue (`codex-rs/config/src/types.rs:47,55-56,339-350`; `codex-rs/memories/write/src/lib.rs:78-84`; `codex-rs/memories/write/src/phase1.rs:149-175,205-220`). If the first sixteen jobs occupy two roughly 31-minute waves, a seventeenth claim can outlive its lease before starting, and a later root-turn startup can reclaim it because the DB cap counts only unexpired leases and stale `running` rows are claimable (`codex-rs/app-server/src/request_processors/turn_processor.rs:673-684`; `codex-rs/state/src/runtime/memories.rs:725-800,1807-1858`). The old future does not heartbeat or revalidate ownership before calling the model; ownership is checked only at final DB update, so reclaimed and stale futures can both reach live calls while only the current token commits (`codex-rs/memories/write/src/phase1.rs:227-277,282-317,327-402`; `codex-rs/memories/write/src/runtime.rs:283-317`; `codex-rs/state/src/runtime/memories.rs:858-895`). Stock settings substantially reduce likelihood—two jobs cannot queue behind an eight-wide buffer, and the five-minute default is an idle-per-event timeout, not a total-call deadline (`codex-rs/model-provider-info/src/lib.rs:29,375-380`; `codex-rs/codex-api/src/endpoint/responses_websocket.rs:685-709`). |
| H102 | **Plausible** | The decisive Codex-backend witness is narrower than the hypothesis but real. `rate_limits_check` returns `None` for absent auth, non-Codex auth, a rate-limit GET/decode error, or no selected snapshot, and `rate_limits_ok` converts every such result to `true`; startup performs that one check and then runs Phase 1 followed by Phase 2 without another quota check (`codex-rs/memories/write/src/guard.rs:9-39`; `codex-rs/memories/write/src/start.rs:64-80`; `codex-rs/backend-client/src/client/rate_limit_resets.rs:23-36`). With valid ChatGPT/Codex-backend auth, an enabled memory configuration, eligible backlog, and a route-specific transient failure from the rate-limit endpoint while the Responses endpoint remains healthy, Phase 1 can claim the configured batch and send one logical extraction sample for every claim; one job failure does not abort the remaining stream (`codex-rs/memories/write/src/phase1.rs:70-95,149-220,227-317`; `codex-rs/memories/write/src/runtime.rs:242-319`). The default batch is 2 and the enforced maximum is 128, processed eight at a time; after Phase 1, an available Phase-2 lease plus changed/invalid workspace artifacts can dispatch one consolidation agent with no intervening recheck (`codex-rs/config/src/types.rs:47-56,339-399`; `codex-rs/memories/write/src/lib.rs:78-107`; `codex-rs/memories/write/src/phase2.rs:49-202,292-378`). Provider defaults make those extraction/consolidation models Luna and Terra, but explicit memory-model settings and provider-specific IDs replace them (`codex-rs/model-provider/src/provider.rs:129-169`; `codex-rs/memories/write/src/startup_tests.rs:673-836`). Thus one failed preflight can admit up to 128 Phase-1 logical model requests plus one eligible Phase-2 job, excluding transport retries and the separate possibility of multiple requests inside that consolidation job. |
| H103 | **Plausible** | The split in the hypothesis is real. Finalization joins an assistant message, strips `<oai-mem-citation>` from the client-facing text, and attaches parsed `MemoryCitation`; the completed-item path then records the original unmodified `ResponseItem`, not the finalized turn item (`codex-rs/core/src/stream_events_utils.rs:329-358,451-465`). Recording stamps/prepares that original item, appends it to active history, persists it as a rollout response item, and emits it to raw-response observers; ordinary assistant messages are cloned unchanged by `for_prompt`, so the hidden block remains in logical model context on later sampling steps and turns (`codex-rs/core/src/session/mod.rs:3059-3229,3592-3600`; `codex-rs/core/src/context_manager/history.rs:207-223,459-537`; `codex-rs/core/src/session/turn.rs:371-390`). Its immediate machine purposes have already occurred: the structured citation is exposed on `AgentMessageItem` and its rollout IDs update stage-1 usage accounting (`codex-rs/protocol/src/items.rs:142-158`; `codex-rs/core/src/stream_events_utils.rs:91-129`). More decisively, a later memory worker loads the complete append-only rollout, ignores `Compacted` markers, and clones every non-user assistant message unchanged into a fresh phase-1 request; its only message-content filtering removes developer messages and selected contextual **user** fragments, not assistant citation markup (`codex-rs/memories/write/src/phase1.rs:282-319,404-466`). Memory tools return non-external `JsonToolOutput`, and startup claims explicitly select `memory_mode = 'enabled'`, so using memories does not itself exclude such a rollout from later extraction (`codex-rs/ext/memories/src/tools/read.rs:73-93`; `codex-rs/tools/src/tool_output.rs:89-130`; `codex-rs/state/src/runtime/memories.rs:133-284`). Thus citation paths, notes, and rollout IDs deterministically add phase-1 input and can be treated as evidence again; whether the model copies them into durable memory is nondeterministic. The claimed “on every later request” needs narrowing: a compatible Responses WebSocket chain reuses `previous_response_id` across turns and transmits only the suffix, while HTTP, reconnect, changed-request, and non-prefix paths replay full input (`codex-rs/core/tests/suite/client_websockets.rs:1335-1430,2332-2363`). The old citation nevertheless remains server-referenced model context until compaction. Raw preservation has legitimate rollout/replay, raw-event, and incremental-prefix provenance value, so the defect is the absence of a sanitized model/extraction projection, not necessarily retention of the sole raw representation. |
| H104 | **Plausible** | Phase 2 starts an ephemeral internal `MemoryConsolidation` thread and submits a prompt whose required outputs are files under the memory root, not terminal message content (`codex-rs/memories/write/src/phase2.rs:182-204,292-378`; `codex-rs/memories/write/templates/memories/consolidation.md:179-197,852-880`). When the model emits a terminal assistant `Message`, core extracts its visible text as `last_agent_message`, records the response item in the thread's in-memory history, and then records the provider-returned completion usage (`codex-rs/core/src/stream_events_utils.rs:60-105,237-360`; `codex-rs/core/src/session/turn.rs:2247-2253,2300-2431,2581-2619`). Task completion puts that text into `TurnCompleteEvent`; event delivery derives `AgentStatus::Completed(Some(text))` from it (`codex-rs/core/src/tasks/regular.rs:73-95`; `codex-rs/core/src/tasks/mod.rs:590-599,812-836`; `codex-rs/core/src/agent/status.rs:4-22`; `codex-rs/core/src/session/mod.rs:2237-2262`). The Phase 2 monitor never reads the text: it only matches `Completed(_)`, reads aggregate token usage for metrics, shuts the thread down, and validates the file artifacts (`codex-rs/memories/write/src/phase2.rs:395-488,492-555,578-605`). Because the worker is configured `ephemeral`, it has no durable `LiveThread` rollout; shutdown emits `ShutdownComplete`, overwrites the live status, and removes the thread from the manager (`codex-rs/memories/write/src/phase2.rs:292-320`; `codex-rs/core/src/session/session.rs:812-920`; `codex-rs/memories/write/src/runtime.rs:322-378`; `codex-rs/core/src/session/handlers.rs:441-478`; `codex-rs/core/src/thread_manager.rs:1154-1159`). Success is independently determined by `MEMORY.md`, a `memory_summary.md` beginning with `v1`, workspace-baseline reset, and DB state; tests use the same `"phase2 complete"` answer for both a successful seeded-artifact run and a failed missing-artifact run, showing that recap text neither proves nor repairs the result (`codex-rs/memories/write/src/workspace.rs:50-76`; `codex-rs/memories/write/src/startup_tests.rs:419-463,803-835,1133-1169`). Thus a generated terminal recap is usage-accounted but unnecessary to the ordinary file-based result contract. The claim needs one qualification: managed/system policy or executor Stop hooks receive `last_assistant_message` before `TurnComplete` and may reject consolidation, so the text is not literally consumerless in every configured environment (`codex-rs/core/src/session/turn.rs:502-520`; `codex-rs/core/src/hook_runtime.rs:312-385`; `codex-rs/hooks/src/events/stop.rs:42-86,120-166`; `codex-rs/memories/write/src/startup_tests.rs:180-320`). Terra's low default verbosity makes the usual waste modest, but the consolidation template supplies no exact sentinel or no-recap instruction, so it is only softly bounded rather than eliminated (`codex-rs/model-provider/src/provider.rs:128-169`; `codex-rs/models-manager/models.json:135-204`). |
| H105 | **Plausible** | Phase 1 does convert retained typed media into model-visible base64 text. User image/audio inputs become `InputImage`/`InputAudio` items backed by prepared data URLs, and media-bearing function/custom-tool outputs retain equivalent typed content-item arrays. The conversation recorder persists those response items losslessly in rollout JSONL, and the memory policy admits messages plus function/custom-tool outputs (`codex-rs/protocol/src/models.rs:851-869,1955-2059,2118-2203`; `codex-rs/protocol/src/local_media.rs:13-43`; `codex-rs/core/src/session/mod.rs:3067-3104,3175-3220`; `codex-rs/rollout/src/policy.rs:40-90`). Memory extraction then loads every raw rollout line, removes only selected text fragments from user messages, clones image/audio content unchanged, JSON-serializes the retained `ResponseItem` vector, and inserts that JSON into one outer `ContentItem::InputText`. Request construction clones and serializes that outer text item; nothing parses the nested JSON back into typed media, and optional HTTP zstd compression changes only transfer encoding (`codex-rs/memories/write/src/phase1.rs:282-319,404-468`; `codex-rs/core/src/client_common.rs:17-65`; `codex-rs/core/src/client.rs:893-990,1526-1534,1595-1611`; `codex-rs/codex-api/src/common.rs:302-330`; `codex-rs/codex-api/src/endpoint/responses.rs:135-183`). Compaction replaces active history but merely appends a `Compacted` checkpoint; the raw loader keeps all prior lines, while Phase 1 ignores the checkpoint and still serializes the original media-bearing response items. Thus media already absent from reconstructed active history can reappear as text in extraction (`codex-rs/core/src/session/mod.rs:3496-3542`; `codex-rs/rollout/src/recorder.rs:1026-1088`; `codex-rs/core/src/session/rollout_reconstruction.rs:187-223,370-418`; `codex-rs/core/src/compact.rs:510-515,540-569`). The size claim also reproduces: 1 MiB yields 1,398,104 base64 characters, or 1,398,126 characters with a `data:image/png;base64,` prefix, which the repository's `ceil(UTF-8 bytes / 4)` heuristic counts as about 349,532 tokens (`codex-rs/utils/image/src/lib.rs:58-68`; `codex-rs/utils/string/src/truncate.rs:4,15-35,71-78`). Under current default Luna metadata, Phase 1 permits `272,000 * 95% * 70% = 180,880` heuristic rollout tokens, so that one payload is middle-truncated but can still saturate essentially the entire rollout allowance with base64 prefix and suffix rather than useful conversation text (`codex-rs/model-provider/src/provider.rs:127-135`; `codex-rs/models-manager/models.json:261-292`; `codex-rs/memories/write/src/prompts.rs:96-126`). Secret redaction is not media-aware, and the occurrence is gated but material: `MemoryTool` is default-disabled; when enabled, eligible root/non-ephemeral, state-backed, rate-limit-approved threads default to two claimed rollouts per startup and can be configured up to 128 (`codex-rs/secrets/src/sanitizer.rs:3-21`; `codex-rs/features/src/lib.rs:1035-1040`; `codex-rs/memories/write/src/start.rs:20-76`; `codex-rs/config/src/types.rs:47-56,339-399`; `codex-rs/state/src/runtime/memories.rs:226-260`). |
| H106 | **Plausible** | A realistic finite WebM enters the claimed fallback. FFmpeg 9.0.1's live WebM muxer (`-t 10 -c:a libopus -b:a 64k -f webm -live 1`) produced a 123,355-byte file that decoded cleanly through 501 packets ending at 10.00 seconds, while both FFprobe and Symphonia reported no container duration. A focused probe against the current crates showed audio preparation accepted the resulting data URL unchanged, Symphonia opened it successfully with a 1/1000 time base but `track.duration=None` and `media_info().duration=None`, and `estimate_audio_token_count` exactly equaled `approx_token_count(audio_url)`: 41,125 tokens instead of the intended 100, about 411x. The same source written as ordinary seekable WebM carried a 10.008-second media duration and estimated 101 tokens, isolating missing duration metadata rather than corrupt media. Preparation cannot repair the case: the protocol audio variants contain only `audio_url`, and preparation validates the data-URL MIME, base64 syntax, and 50 MiB size cap before only canonicalizing the prefix; it neither parses nor stores duration (`codex-rs/protocol/src/models.rs:853-865,2040-2055`; `codex-rs/utils/audio/src/lib.rs:94-186,268-350`). The impact also reproduces. Checked-in token-mode models use a 10,000-token output policy, history raises that to 12,000 for serialization headroom, and the multimodal truncator drops an audio item whose estimated cost exceeds the remaining budget; the focused probe converted the valid 10-second WebM to `[omitted 1 audio items ...]` at 12,000 (`codex-rs/models-manager/models.json:141-149`; `codex-rs/core/src/context_manager/history.rs:479-514,674-684`; `codex-rs/protocol/src/protocol.rs:3250-3260`; `codex-rs/utils/output-truncation/src/lib.rs:94-184`). A 3-second live WebM likewise had no duration, estimated 11,551 tokens instead of 30, and survived the 12,000-token admission gate. Because completed tool output is recorded after a model-generated call and added locally on top of server-reported usage, that retained phantom cost can cross the auto-compaction threshold while a tool call already requires follow-up, causing a premature mid-turn compaction (`codex-rs/core/src/context_manager/history.rs:416-456,992-1013`; `codex-rs/core/src/stream_events_utils.rs:297-321`; `codex-rs/core/src/session/turn.rs:413-473,2777-2786`; `codex-rs/core/src/session/context_window.rs:23-91`). Exposure is conditional: all checked-in model presets currently advertise only text and image, but audio is a supported model modality and the app-server integration suite constructs an audio-capable model and proves structured tool audio reaches the follow-up request (`codex-rs/protocol/src/openai_models.rs:169-180`; `codex-rs/models-manager/models.json:10-18,141-149,267-275,389-397,507-515,619-627,735-743,852-860,964-972,1072-1080`; `codex-rs/app-server/tests/suite/v2/dynamic_tools.rs:599-604,690-801`). |
| H107 | **Plausible** | A usage-bearing tool-call completion records the provider's `last_token_usage` before pending tool futures are drained and their outputs are appended to history (`codex-rs/core/src/session/turn.rs:2568-2608,2770-2788`; `codex-rs/core/src/session/turn.rs:2155-2165`). Those post-model outputs are then added heuristically to active usage from their raw stored representation, including full image/audio estimates (`codex-rs/core/src/context_manager/history.rs:416-456,700-764,856-898,992-1010`). The post-sampling check runs after tool drain and immediately starts mid-turn compaction when that total reaches the auto-compaction or full-window boundary (`codex-rs/core/src/session/turn.rs:404-483`; `codex-rs/core/src/session/context_window.rs:23-79`). Only after that decision do the normal and both local/remote compaction request paths clone history and replace unsupported image/audio with short text markers (`codex-rs/core/src/session/turn.rs:371-390`; `codex-rs/core/src/compact.rs:277-285`; `codex-rs/core/src/compact_remote_request.rs:41-67`; `codex-rs/core/src/context_manager/normalize.rs:328-417`). Dynamic tools provide a concrete producer: their response media is mapped directly to high-detail image/audio function-output items without filtering against the active model's modalities, and an integration test proves those items reach a follow-up request (`codex-rs/core/src/tools/handlers/dynamic.rs:153-164`; `codex-rs/protocol/src/models.rs:2095-2117`; `codex-rs/app-server/tests/suite/v2/dynamic_tools.rs:691-805`). For shipped `gpt-5.4`, audio is unsupported, direct tools are enabled, a 10K-token output policy is expanded to a 12K storage allowance, and the 272K window has a 244.8K default auto-compaction boundary (`codex-rs/models-manager/models.json:729-756`; `codex-rs/core/src/context_manager/history.rs:479-508`; `codex-rs/protocol/src/openai_models.rs:503-519`). Thus a valid duration-bearing approximately 1,199-second audio output can contribute about 12K local-tail tokens and push provider usage near 232.8K over the boundary, while the transmitted compaction/follow-up request contains only the tiny unsupported-audio marker and remains far below it. One unnecessary compaction invocation is therefore reachable. |
| H108 | **Plausible** | Guardian V2 can send one exact REPL screenshot twice in a single Luna classifier request, although a nested screenshot is not duplicated automatically. A successful Code Mode call to a `node_repl`- or `cua_repl`-backed MCP server records its image in thread-scoped `NodeReplReviewEvidence`; the evidence accessor deduplicates repeated evidence images only against other evidence images by exact `image_url` (`codex-rs/core/src/tools/handlers/mcp.rs:279-389`; `codex-rs/core/src/context/node_repl_review_evidence.rs:91-126`). The same Code Mode script can explicitly forward that MCP image with `image(imageItem, "high")` or `"original"`, preserving its pixels as an image in the outer `exec` custom-tool output and therefore in conversation history (`codex-rs/code-mode-protocol/src/description.rs:13-14,27-31`; `codex-rs/code-mode-runtime/src/runtime/value.rs:44-96,129-171`; `codex-rs/code-mode-runtime/src/service_tests.rs:1308-1342`; `codex-rs/core/tests/suite/code_mode.rs:4201-4249`). On a later classified tool call, Guardian V2 snapshots the retained evidence and independently scans history. `TranscriptConfig::images` appends history images first and REPL-evidence images second using only a four-image/eight-MiB eviction budget, with no cross-source deduplication (`codex-rs/ext/guardian-v2/src/async_scorer/extension.rs:476-532`; `codex-rs/ext/guardian-v2/src/async_scorer/transcript.rs:25-26,82-157`). The sampler then appends every collected image to one user message and clears both detail fields; for a supported PNG that needs no resize, the final content therefore ends with two byte-identical objects, `{"type":"input_image","image_url":"data:image/png;base64,<same>"}`, with no source label or semantic distinction (`codex-rs/utils/image/src/lib.rs:200-245`; `codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs:434-544`; `codex-rs/ext/guardian-v2/src/async_scorer/extension_tests.rs:1631-1696`). Two suitably compressed screenshots fit the collector limits while still carrying meaningful vision cost: the repository estimates a detail-less/non-original image at about 1,844 tokens and original-detail processing by 32-pixel patches up to 10,000 tokens (`codex-rs/core/src/context_manager/history.rs:700-717,836-863`). Retryable failures reuse the same request input for up to two retries after the initial attempt, so the duplicate can be transmitted three times, although charging of failed attempts is provider-dependent (`codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs:372-432,556-647`). |
| H109 | **Plausible** | Both active construction paths accept model-visible Guardian instructions without a mandatory size bound. Synchronous Guardian resolves requirements/config policy first, then reviewer-catalog policy, then the bundled policy; the only normalization is whitespace trimming. Its reviewer-catalog `policy_template` is also an unrestricted string, and every `{{ tenant_policy_config }}` occurrence is replaced before the fixed output contract is appended, with no truncation. The result becomes the child reviewer's custom base instructions and is present verbatim in its startup prewarm request (`codex-rs/core/src/config/mod.rs:1504-1516,3865-3878,4685-4690`; `codex-rs/core/src/guardian/prompt.rs:806-844`; `codex-rs/core/src/guardian/review_session.rs:1378-1411`; `codex-rs/core/src/client.rs:899-985`; `codex-rs/core/tests/suite/guardian_review.rs:380-429`). Guardian V2 likewise resolves local `classifier_instructions`, then parent-model catalog defaults, then the bundled template. `max_classifier_instruction_tokens` is optional and defaults to `None`; only an explicitly supplied local or model cap truncates the fully rendered classifier after all policy substitutions, and the uncapped result is inserted as a complete Luna developer message on every classification request (`codex-rs/ext/guardian-v2/src/async_scorer/config.rs:34-43,73-103,154-175,231-244,280-304`; `codex-rs/ext/guardian-v2/src/async_scorer/extension.rs:598-636`; `codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs:434-515`). Focused tests assert both exact uncapped outbound preservation and bounded behavior only when the optional cap is configured (`codex-rs/ext/guardian-v2/src/async_scorer/extension_tests.rs:2370-2435`; `codex-rs/ext/guardian-v2/src/async_scorer/config_tests.rs:56-75,150-179,184-246`). Using the repository's `ceil(UTF-8 bytes / 4)` estimate, the checked-in LF bundled synchronous policy, template, and output contract render to 18,446 bytes, about 4,612 tokens, so H109's approximately 4.5K default figure is accurate; this Windows checkout's CRLF `include_str!` inputs make it about 4,647. A configured policy/template/classifier can be arbitrarily larger, repeat once per placeholder, and exceed Luna's finite 258,400-token effective normal input window before any action, transcript, tools, or framing. Provider/transport rejection is a failure boundary, not a client-side cap or truncation. New routed parent sessions prewarm a fresh reviewer, policy/template changes alter the reuse key and replace the reviewer, and Guardian V2 resends the complete request on each eligible tool classification and on up to two recoverable retries. Reusable synchronous WebSocket follow-ups can send only an incremental suffix and prompt caching may reduce backend work, so “resent on retries” is not universal, but the uncapped initial/new-session paths and full V2 replay make the core claim plausible. |
| H110 | **Plausible** | Goal attachment materialization is real semantic indirection, not just persistence. `GoalDraft` carries the objective plus pending paste payloads and local images, but materialization writes each active paste to `$CODEX_HOME/attachments/<uuid>/pasted-text-N.txt`, copies each local image there, and persists only path text; an objective above 4,000 characters is replaced wholesale by `Read the Codex goal objective file at ... before continuing.` (`codex-rs/tui/src/goal_files.rs:17-29,33-136`; `codex-rs/protocol/src/protocol.rs:3939-3950`). The path is not automatically dereferenced into model input. Setting an active goal immediately calls `continue_if_idle`, and every later idle callback reloads the persisted goal and starts another turn whose internal user message interpolates that same objective/reference (`codex-rs/ext/goal/src/runtime.rs:165-213,363-415`; `codex-rs/ext/goal/src/extension.rs:148-159`; `codex-rs/ext/goal/src/steering.rs:45-79`). Therefore, when the objective is wholly file-backed, a content-aware response necessarily depends on at least one model tool call and its result; a pasted fragment or image similarly requires a file/image fetch before its content is known. This cost is not confined to unsafe payloads: a 1,001-character paste is materialized even though ordinary submission expands it inline, and a 4,001-character typed objective is only one character over the goal-field limit while ordinary user text is allowed up to 1,048,576 characters (`codex-rs/tui/src/bottom_pane/chat_composer.rs:343-345,1193-1212,2972-3012`; `codex-rs/protocol/src/user_input.rs:8-10`; `codex-rs/tui/src/chatwidget/tests/goal_validation.rs:100-127,152-183`). Tool outputs are recorded in logical conversation history, yet each automatic continuation reissues the imperative reference and the continuation template tells the model to treat current state as authoritative and inspect referenced files during completion audit; that materially encourages rereading content already present in the active history (`codex-rs/core/src/hook_runtime.rs:638-662`; `codex-rs/core/src/session/mod.rs:3174-3227`; `codex-rs/core/src/context_manager/history.rs:175-204,459-530`; `codex-rs/ext/goal/templates/goals/continuation.md:1-20,139-156`). Rereading is not runtime-enforced and a fetch can share a parallel tool batch with necessary work, so this does not prove one extra wall-clock batch on every goal, but the mandatory content dependency and repeated prompt incentive make the reported waste path reachable. |
| H111 | **Unlikely** | Current Remote V2 does place normalized tool calls and outputs from the active, not-yet-compacted window in the logical compaction prompt, and its overflow rewrite is narrowly limited: it starts only when the local estimate exceeds the model's usable context window, walks newest-to-oldest, rewrites `FunctionCallOutput`, `CustomToolCallOutput`, or `ToolSearchOutput`, and stops at the first other item (`codex-rs/core/src/compact_remote_v2_attempt.rs:41-85`; `codex-rs/core/src/compact_remote.rs:399-507`; `codex-rs/core/src/compact_remote_history.rs:10-40`; `codex-rs/core/src/session/turn_context.rs:432-436`). However, the central waste claim does not survive. A successful V2 checkpoint removes tool calls, tool outputs, assistant messages, and the previous compaction item, retaining bounded user/developer/agent messages plus the newly generated opaque `Compaction` item; therefore outputs presented to a compaction have not already been captured by an earlier checkpoint and do not recur in later successful windows (`codex-rs/core/src/compact_remote_v2.rs:480-567`; `codex-rs/core/src/compact_remote_v2.rs:815-848`). No per-output semantic summary or consumed-evidence marker exists before that new compaction item is generated. File contents, command/test results, searches, and MCP data may exist only in their tool outputs, so removing them before the model produces the replacement summary would lose context rather than eliminate proven redundancy (`codex-rs/core/src/session/turn.rs:143-153`; `codex-rs/core/src/tools/context.rs:99-177,337-380,540-566`). Large volume is possible—the checked-in Sol policy is 10,000 tokens and history allows a 1.2 serialization budget, while tool-search output is capped at 32 KiB—but that establishes material compaction input, not unnecessary rereading (`codex-rs/models-manager/models.json:4-20`; `codex-rs/core/src/context_manager/history.rs:479-527,613-654`; `codex-rs/tools/src/tool_discovery.rs:8-18`). In addition, “full context limit” is imprecise: ordinary auto-compaction defaults to 90% of the resolved window, whereas this rewrite compares against the 95%-usable window, and eligible WebSocket continuations can transmit only the incremental suffix under `previous_response_id` rather than retransmitting the earlier request (`codex-rs/protocol/src/openai_models.rs:499-521`; `codex-rs/core/src/client.rs:1311-1390,1817-1859`). |
| H112 | **Plausible** | A synchronous Stop/SubagentStop hook that blocks completion turns its nonempty reason into a `HookPromptFragment` whose `hook_run_id` is the completed hook-run ID; the hook runtime caps each fragment at 2,500 tokens but does not impose an aggregate lifecycle cap (`codex-rs/hooks/src/events/stop.rs:250-409,413-446`; `codex-rs/hooks/src/engine/mod.rs:147-159,449-455`; `codex-rs/hooks/src/output_spill.rs:12-25,107-119`). The turn loop serializes those fragments as XML content in a `user`-role `ResponseItem::Message`, with no content-kind passthrough metadata, records it in canonical/persisted history, and immediately continues sampling (`codex-rs/protocol/src/items.rs:623-674`; `codex-rs/core/src/session/turn.rs:502-539`; `codex-rs/core/src/session/mod.rs:3174-3229,4203-4215`). Remote V2 compaction obtains normalized model history without filtering contextual user messages, then retains user/system/developer candidates and applies the shared installed-history predicate; that predicate explicitly recognizes and preserves `TurnItem::HookPrompt` (`codex-rs/core/src/compact_remote_v2_attempt.rs:68-82,135-143`; `codex-rs/core/src/context_manager/history.rs:207-224,459-478`; `codex-rs/core/src/compact_remote_v2.rs:480-558`; `codex-rs/core/src/compact_remote.rs:354-386`). V2 keeps the newest retained messages under a 64,000-token budget, appends the opaque compaction item, inserts fresh initial context without removing hook prompts, and installs/persists that replacement history (`codex-rs/core/src/compact_remote_v2.rs:75-78,309-353,578-672`; `codex-rs/core/src/compact.rs:576-638`; `codex-rs/core/src/session/mod.rs:3496-3549`). The next sampling request is rebuilt from that installed history, so the old hook prompt remains model-visible (`codex-rs/core/src/session/turn.rs:371-390,1384-1404`). Existing tests prove continuation prompts accumulate across repeated blocks, survive into a later resumed user turn, and are model-visible; V2 tests independently prove retained user messages survive replacement into the follow-up request (`codex-rs/core/tests/suite/hooks.rs:1133-1170,1300-1404,2369-2478,2483-2538`; `codex-rs/core/tests/suite/compact_remote.rs:1482-1809`). |
| H113 | **Plausible** | Remote V2 has an at-least-once gap after a provider has produced the complete compaction item but before the client observes `response.completed`. The collector retains `response.output_item.done` only in a local variable and returns it solely after `ResponseEvent::Completed`; EOF or another retryable stream failure instead returns `CodexErr::Stream`, drops that item, and causes `run_remote_compaction_request_v2` to invoke `client_session.stream` again with the same full logical prompt (`codex-rs/core/src/compact_remote_v2.rs:375-457`; `codex-rs/protocol/src/error.rs:90-95,380-419`). The cited integration test exercises exactly this state transition: one compact response emits `FAILED_COMPACT_SUMMARY` and ends without completion, the next request performs compaction again, and only `RETRIED_COMPACT_SUMMARY` is installed; all three compact attempts retain the same pre-compaction window number (`codex-rs/core/tests/suite/compact_remote.rs:1832-1935`). There is no per-operation idempotency key in the Responses WebSocket body or an `Idempotency-Key` header. The locally generated compaction item ID is lifecycle/trace state and is not transmitted, while turn/window metadata is shared by legitimate successive requests and therefore does not identify one compaction attempt (`codex-rs/codex-api/src/common.rs:359-391`; `codex-rs/core/src/compact_remote_v2.rs:223-250`; `codex-rs/core/src/responses_metadata.rs:210-240`). Nor can the retry continue the lost response: `response.created` discards its response ID, WebSocket `LastResponse` is published only on terminal completion, and resetting the failed socket clears continuation state, so the retry has no `previous_response_id` and sends full nonempty input; focused transport coverage asserts that shape directly (`codex-rs/codex-api/src/common.rs:126-146`; `codex-rs/codex-api/src/sse/responses.rs:493-501`; `codex-rs/core/src/client.rs:1264-1272,1355-1390,1447-1508,2190-2225`; `codex-rs/core/tests/suite/websocket_retry.rs:133-181`). Durability has a second, narrower gap after the client does receive completion: `RawResponseCompleted` is persisted before the replacement checkpoint, but reconstruction ignores that event for model history and recognizes only `RolloutItem::Compacted`; a crash or checkpoint-write failure in between leaves the old history/window, so a resumed over-limit turn or later manual compact can submit the full operation again (`codex-rs/core/src/compact_remote_v2_attempt.rs:105-139`; `codex-rs/core/src/compact_remote_v2.rs:293-359`; `codex-rs/core/src/session/rollout_reconstruction.rs:187-333,362-407`). Once the `Compacted` append succeeds it is synchronously flushed, and loss of only `ItemCompleted`/`TurnComplete` does not recreate the old history, so the crash claim is limited to the pre-checkpoint interval or a swallowed append error (`codex-rs/core/src/session/mod.rs:3496-3543,3866-3872`; `codex-rs/thread-store/src/local/live_writer.rs:309-364`; `codex-rs/core/src/tasks/mod.rs:367-391`). Client code cannot establish the provider's exact billing instant, but if the omitted terminal frame followed server-side completion—as the hypothesis posits—the next unkeyed full request can repeat already completed and billable work. |
| H114 | **Plausible** | Inline Remote V2 reuses the active turn's `ModelClientSession`, so after a completed sample its WebSocket baseline is the preceding full request plus every server-returned output item and response ID, while the same outputs and completed tool results have already entered canonical history (`codex-rs/core/src/session/turn.rs:295-303,371-483,2155-2165,2322-2426,2568-2619,2770-2786`; `codex-rs/core/src/stream_events_utils.rs:76-110,288-382`; `codex-rs/core/src/client.rs:2145-2224`; `codex-rs/core/src/compact_remote_v2.rs:83-106,224-301`). V2 then clones that history, may rewrite trailing tool outputs, appends `CompactionTrigger`, and always sets `output_schema: None`, whereas ordinary sampling uses `TurnContext::final_output_json_schema`; request construction serializes a schema as `text.format`, and the WebSocket continuation gate requires both equal non-input properties—including `text`—and an exact extension of the preceding request plus response items (`codex-rs/core/src/compact_remote_v2_attempt.rs:40-85`; `codex-rs/core/src/session/turn.rs:1332-1344`; `codex-rs/codex-api/src/common.rs:423-440`; `codex-rs/core/src/client.rs:330-385,965-986,1311-1390`). Therefore a reachable structured-output turn, or an actual overflow rewrite, makes continuation fail; the sender omits `previous_response_id` and serializes the complete near-window compaction input instead of only the unsent suffix (normally tool outputs plus the trigger) (`codex-rs/core/src/client.rs:1817-1903`; `codex-rs/core/src/compact_remote.rs:399-507`; `codex-rs/core/tests/suite/client_websockets.rs:2181-2363`). Successful V2 compaction then removes the trigger, filters/reorders the old window to bounded retained messages, appends the new opaque compaction item, injects fresh turn context, and atomically replaces history; the turn immediately samples again (`codex-rs/core/src/compact_remote_v2_attempt.rs:118-143`; `codex-rs/core/src/compact_remote_v2.rs:294-353,480-567`; `codex-rs/core/src/session/mod.rs:3496-3549`; `codex-rs/core/src/session/turn.rs:451-483`). That replacement is not an extension of the compaction request plus its response, so the first post-checkpoint sample deterministically sends another full create—this time the smaller compacted window. The pre-compaction replay is avoidable only in the schema-only case if the private `compaction_trigger` service accepts lineage while `text.format` changes; it is required when earlier input was rewritten. The post-compaction replay is consistent with deliberately starting a client-defined replacement window and cannot safely reuse the compaction response ID without a stronger backend equivalence contract. |
| H115 | **Plausible** | Remote V2 deliberately retains bounded `AgentMessage` instructions after compaction. An agent message survives when its estimated model-visible size is at most 10,000 tokens and it is neither a `FINAL_ANSWER` nor a strict-descendant-to-ancestor `MESSAGE`; consequently `NEW_TASK` delegations/follow-ups, parent-to-child or sibling `MESSAGE`s, and otherwise unclassified agent messages qualify (`codex-rs/core/src/compact_remote_v2.rs:75-78,534-567`). V2 keeps the newest qualifying user/developer/agent messages under a shared 64,000-token allowance, then appends the new opaque compaction item and injects fresh initial context outside that allowance before installing the replacement history (`codex-rs/core/src/compact_remote_v2.rs:309-353,480-509,585-678`; `codex-rs/core/src/session/mod.rs:3496-3549`). The checked-in integration test provides the required large witness: it records a 40,022-byte encrypted delegated task, waits for its triggered turn to finish, compacts, and verifies that the exact ciphertext and original creation timestamp remain in the next model request; the current estimator prices that specimen at about 5.7K tokens (`codex-rs/core/tests/suite/compact_remote.rs:1610-1642,1672-1682,1753-1782`; `codex-rs/core/src/context_manager/history.rs:669-702,727-772,953-984`). Six near-cap messages can therefore occupy about 60K of the 64K retained-message budget. Preserving delegated instructions is intentional continuity, not universally wasteful, but retention has no task-status, completion-pairing, age, or “already consumed” check, so a completed turn does not expire its prior task/message and the claimed stale-retention case remains reachable. |
| H116 | **Unlikely** | The one-shot previous/current-model fallback is real, but the claimed avoidable repetition of a completed compaction is not established. A fallback context exists only for pre-sampling compaction across distinct models under ChatGPT/Codex-backend authentication with the OpenAI provider, and only when a compaction-compatibility hash changed or an over-limit thread is moving to a smaller context window (`codex-rs/core/src/session/turn.rs:1067-1184`). The V2 driver first runs the previous-model attempt and, for the selected error classes, independently rebuilds an attempt under the current model; checked-in tests prove that a rejected or unavailable previous model produces compaction requests to two different model slugs (`codex-rs/core/src/compact_remote_v2.rs:245-292`; `codex-rs/core/src/compact_model_fallback.rs:8-19`; `codex-rs/core/tests/suite/compact.rs:2519-2655,2659-2757`). However, `collect_compaction_output` returns success only after `response.completed` and exactly one compaction item. A valid completed first response therefore ends the operation without fallback, while a malformed completed response is `Fatal` and is not eligible for fallback (`codex-rs/core/src/compact_remote_v2.rs:399-470`; `codex-rs/protocol/src/error.rs:380-419`). The failures most likely to represent ambiguous partial server work—stream closure, response-stream failure, and connection failure—are absent from the cross-model predicate; they remain in the same-model retry path or fail the operation. The closest focused test deliberately emits a compaction item without `response.completed`, proves the whole request is retried, and proves the failed item is discarded, but it retries the same model and does not establish reusable successful output (`codex-rs/core/tests/suite/compact_remote.rs:1832-1933`). A different model also cannot use WebSocket `previous_response_id` continuation because request compatibility includes model equality, and failed responses do not publish a reusable `LastResponse` (`codex-rs/core/src/client.rs:330-385,1311-1389,1817-1854,2205-2274`). Thus two complete attempts can occur after rejection, but no current evidence shows a valid completed compaction being avoidably regenerated; the fallback is the recovery needed to unblock a model switch when the previous model is retired or incompatible. |
| H117 | **Unlikely** | Remote V2 unquestionably ignores non-compaction output items, but the cited evidence does not establish the required paid, avoidable production witness. The request appends a typed `CompactionTrigger` as its final item and supported providers are declared only when they implement that Responses compaction contract (`codex-rs/core/src/compact_remote_v2_attempt.rs:65-85`; `codex-rs/model-provider/src/provider.rs:45-51,344-350`; `codex-rs/model-provider/src/amazon_bedrock/mod.rs:193-204`). The collector scans every `OutputItemDone`, retains exactly one `ResponseItem::Compaction`, and carries aggregate completion usage forward; replacement history is then rebuilt from pre-request input plus that compaction item, so an extra assistant/reasoning item is not installed (`codex-rs/core/src/compact_remote_v2.rs:302-353,417-509`). However, the cited integration test manufactures `IGNORED_COMPACT_REPLY` in a mock stream and its shared `ev_completed` helper reports **zero** output tokens, so it proves tolerance and discarding, not provider generation or billing (`codex-rs/core/tests/suite/compact_remote.rs:1938-2013`; `codex-rs/core/tests/common/responses.rs:735-743`). A newer unit test pairs the same synthetic assistant item with arbitrary `output_tokens: 42` and `usage_metadata.amount: 0.125`, but its in-memory channel does not derive either value from the item or contact a provider (`codex-rs/core/src/compact_remote_v2.rs:797-811,1144-1203`). The normal request preserves base instructions, tools, and reasoning settings and exposes no `max_output_tokens` field, but there is no evidence that OpenAI, Azure, or Bedrock actually emits assistant text for a final compaction trigger, nor that a lower generic output cap can suppress only such text without truncating the required opaque compaction (`codex-rs/core/src/client.rs:865-989`; `codex-rs/codex-api/src/common.rs:302-329`). Finally, a successfully completed mixed stream returns immediately; retry or model fallback can multiply output only if it was emitted before an error, and the retry test supplies neither completion usage nor a charge for its discarded partial item (`codex-rs/core/src/compact_remote_v2.rs:245-292,368-415`; `codex-rs/core/tests/suite/compact_remote.rs:1832-1935`). Thus discarding is real, but generated-and-billed auxiliary output and an avoidable client-side cause remain unproved. |

### H001. Fresh automation recurrences lose cross-run prompt-cache affinity

- **Lane:** `automations`
- **Agent confidence:** likely
- **Waste class:** background,cumulative,cache-loss
- **Evidence:** app-server/tests/suite/v2/client_metadata.rs:66-89; core/src/thread_manager.rs:1594-1620; core/src/session/session.rs:1181-1205,1539-1548; core/src/session/turn.rs:1328-1341; core/src/client.rs:514-525,970-985; app-server-protocol/src/protocol/v2/plugin.rs:730-734
- **Hypothesis:** Stable scheduled-task identity is not connected to thread execution or prompt_cache_key; every recurrence uses a new session cache key while rebuilding the same bootstrap prefix.
- **Claimed impact:** At least base context plus up to 32KiB AGENTS, about 8K tokens, and other stable context per recurrence.
- **Aggregation note:** BUGS.md #19/#20 discuss catalog/compaction cache loss, not automation recurrence identity. Provider-dependent realized misses.

### H002. Automation trigger identifiers are not idempotency keys

- **Lane:** `automations`
- **Agent confidence:** confirmed
- **Waste class:** retry-triggered,multiplicative
- **Evidence:** app-server/src/request_processors/turn_processor.rs:601-607; core/src/session/mod.rs:4219-4237; core/src/session/turn_input.rs:275-318,630-636; core/src/session/turn.rs:413-425; state/src/runtime/queued_items.rs:77-104; state/src/lib.rs:99-100
- **Hypothesis:** fiber_run_id is metadata and clientUserMessageId is emitted but not checked; ambiguous retries become steering or fresh turns, while queue retries get new UUIDs.
- **Claimed impact:** One extra inference per direct retry or up to 100 duplicate queued turns.
- **Aggregation note:** Distinct from known over-frequency issue and BUGS.md retry findings. Strong novel candidate.

### H003. Queue execution starts before durable dequeue

- **Lane:** `automations`
- **Agent confidence:** conditional
- **Waste class:** retry-triggered,background
- **Evidence:** ext/queue/src/service.rs:368-401,439-448,549-563; core/src/tasks/mod.rs:280-364
- **Hypothesis:** The model turn is spawned before deleting the durable queue record; deletion failure or process loss leaves an already-executing item available for replay on next idle/resume.
- **Claimed impact:** At least one duplicate turn; unbounded repeats if deletion keeps failing while execution succeeds.
- **Aggregation note:** Distinct from trigger idempotency and known automation frequency. Failure window is confirmed.

### H004. Durable automation rollouts are eligible for memory extraction

- **Lane:** `automations`
- **Agent confidence:** conditional
- **Waste class:** background,cumulative
- **Evidence:** app-server-protocol/src/protocol/v2/thread.rs:108-113; core/src/config/mod.rs:4193; memories/write/src/start.rs:20-37; memories/write/src/phase1.rs:153-176,315-317; state/src/runtime/memories.rs:176-234
- **Hypothesis:** Automation does not force ephemeral and memory startup does not exclude automation sources, so scheduled monitor/reminder histories can receive separate background extraction model calls.
- **Claimed impact:** Default up to two extraction calls per memory startup, plus possible consolidation.
- **Aggregation note:** Distinct from BUGS.md #14 Phase2 retry; requires memory feature and durable automation. Need verify whether memory semantics intentionally include automation.

### H005. Non-thread workers inherit ordinary model-context hooks

- **Lane:** `hooks`
- **Agent confidence:** confirmed
- **Waste class:** background,cumulative,multiplicative
- **Evidence:** memories/write/src/phase2.rs:312-338; memories/write/src/runtime.rs:322-337; core/src/tasks/review.rs:105-136; core/src/session/mod.rs:4379-4384; core/src/hook_runtime.rs:121-162,600-625,948-957; core/src/session/turn.rs:264-268,371-390
- **Hypothesis:** Memory and review workers inherit parent hooks; session/prompt/tool/compact hooks are not source-gated and inject context into worker requests.
- **Claimed impact:** About 2500 tokens per inherited fragment under defaults, multiplied by worker prompts/tools; async output can add worker inference.
- **Aggregation note:** Distinct from BUGS.md #8 generic limits and fixed memory Stop hook. Corroborates first-pass candidate omitted from BUGS.md.

### H006. Rollback can resurrect async hook context from deleted turns

- **Lane:** `hooks`
- **Agent confidence:** confirmed
- **Waste class:** background,cumulative,retry-triggered
- **Evidence:** hooks/src/engine/dispatcher.rs:254-267; hooks/src/engine/command_runner.rs:107-179; core/src/session/handlers.rs:244-327,398-406; core/src/hook_runtime.rs:697-716; core/tests/suite/hooks.rs:1718-1844
- **Hypothesis:** Rollback reconstructs history without cancelling session-owned async hook work; drain ignores originating turn_id and injects late output into the new turn.
- **Claimed impact:** One stale context payload per late result, resent until compaction; may force an extra sampling iteration.
- **Aggregation note:** Distinct from BUGS.md #12 Code Mode stale notifications and #8 async hook waves. Strong novel lifecycle candidate.

### H007. Single Stop cycle aggregates unbounded per-handler fragments

- **Lane:** `hooks`
- **Agent confidence:** confirmed
- **Waste class:** per-request,multiplicative
- **Evidence:** hooks/src/events/stop.rs:391-445,652-684; hooks/src/output_spill.rs:12-22,107-118; protocol/src/items.rs:623-643; core/src/session/turn.rs:521-529
- **Hypothesis:** Each blocking handler gets a separate 2500-token limit, then all fragments combine into one user message with no aggregate cap or final admission check.
- **Claimed impact:** Four full fragments exceed 10K before framing; handler count unbounded.
- **Aggregation note:** Distinct from BUGS.md #9 repeated Stop cycles and #8 generic developer hook context. Strong.

### H008. Unified Exec promotes raw terminal noise and stdin echo to model text

- **Lane:** `unified-exec`
- **Agent confidence:** confirmed
- **Waste class:** per-write,cumulative,multiplicative
- **Evidence:** core/src/unified_exec/process.rs:337-347,588-618; core/src/tools/context.rs:454-479; utils/output-truncation/src/lib.rs:17-34; core/tests/suite/unified_exec.rs:2357-2360,3028-3031
- **Hypothesis:** Raw PTY/pipe bytes use lossy UTF-8 and truncation only; stdin already present in tool call is echoed again, while ANSI redraw/control/binary bytes consume model-visible output.
- **Claimed impact:** An S-token write can add roughly S duplicate echo tokens; terminal noise can consume full per-response allowance.
- **Aggregation note:** BUGS.md #2/#4 cover output caps/polling, not redundant raw terminal representation. Novel.

### H009. Unified Exec process handles diverge from durable model history

- **Lane:** `unified-exec`
- **Agent confidence:** confirmed
- **Waste class:** background,retry-triggered,duplicate-execution
- **Evidence:** core/src/unified_exec/process_manager.rs:540-575,775-780; core/src/tools/parallel.rs:243-258; core/src/session/handlers.rs:397-410; core/src/session/session.rs:1355-1362; core/src/session/rollout_reconstruction.rs:378-428; core/src/context/turn_aborted.rs:10-11
- **Hypothesis:** Interrupt can preserve a live process while returning no session ID; resume/shutdown removes processes while restored history still advertises running session IDs, causing failed polls and command reruns.
- **Claimed impact:** At least one failed write/poll and full model recovery turn; often command re-execution and another follow-up.
- **Aggregation note:** Distinct from BUGS.md #4 repeated polling. Strong lifecycle mismatch.

### H010. Fixed post-exit drain can lose final output and force reruns

- **Lane:** `unified-exec`
- **Agent confidence:** confirmed
- **Waste class:** retry-triggered,duplicate-execution
- **Evidence:** core/src/unified_exec/process_manager.rs:1402-1470,1006-1017; utils/pty/src/process.rs:412-422; rollout/src/policy.rs:137-169; core/src/session/rollout_reconstruction.rs:419-427
- **Hypothesis:** Only up to 50ms drains after exit; delayed tail bytes can be lost before session removal, and transient completion events do not repair model history.
- **Claimed impact:** Lost test/error/sentinel tail can force complete command rerun plus model follow-up.
- **Aggregation note:** Distinct from output retention limits; this output never reaches model. Novel.

### H011. Default non-TTY session rejects advertised write_stdin interaction

- **Lane:** `unified-exec`
- **Agent confidence:** confirmed
- **Waste class:** retry-triggered,extra-call
- **Evidence:** core/src/tools/handlers/shell_spec.rs:44-46,91-100,141-145; core/src/unified_exec/process_manager.rs:239,856-862,1284
- **Hypothesis:** Tool descriptions advertise ongoing PTY interaction, but tty defaults false and non-TTY launches are not writable; a valid-looking write_stdin fails and instructs command rerun with tty=true.
- **Claimed impact:** One failed tool call and inference, typically repeated exec and follow-up.
- **Aggregation note:** Distinct from BUGS.md #4 timeout polling. Novel contract-driven token burn.

### H012. Reusable Guardian delta turns append full non-delta evidence

- **Lane:** `reasoning-reviewers`
- **Agent confidence:** confirmed
- **Waste class:** background,cumulative,retry-triggered
- **Evidence:** core/src/guardian/review_session.rs:900-906,1245-1247; core/src/guardian/prompt.rs:213-265,316-325; core/src/guardian/review.rs:78,1042-1073; core/src/guardian/mod.rs:74; core/src/guardian/approval_request.rs:233-265
- **Hypothesis:** After first review, delta mode still appends complete root authorization snapshot, all trusted answers and denied-read restrictions; invalid completed responses advance count and retries append same action again.
- **Claimed impact:** Root evidence up to about 7200 tokens plus another 7200 trusted answers; denied-read context aggregate-unbounded.
- **Aggregation note:** Distinct from BUGS.md #31 unused reasoning summaries. Strong.

### H013. Guardian V2 duplicates current tool action in classifier prompt

- **Lane:** `reasoning-reviewers`
- **Agent confidence:** confirmed
- **Waste class:** background,per-call
- **Evidence:** core/src/stream_events_utils.rs:316; core/src/tools/lifecycle.rs:27-38; ext/guardian-v2/src/async_scorer/transcript.rs:210-227; ext/guardian-v2/src/extension.rs:578-598; ext/guardian-v2/src/extension_tests.rs:1944-1958
- **Hypothesis:** Persisted tool call appears in rendered transcript and same payload is appended again as planned-action JSON.
- **Claimed impact:** Up to about 1K duplicate tokens by default, configurable as high as 100K.
- **Aggregation note:** Distinct from BUGS.md #21 review output duplication. Direct duplicate.

### H014. Compaction requests reasoning summaries that all consumers discard

- **Lane:** `reasoning-reviewers`
- **Agent confidence:** confirmed
- **Waste class:** output,retry-multiplicative
- **Evidence:** core/src/compact.rs:742-750; core/src/compact_remote_v2.rs:382-448; core/src/compact_remote_request.rs:86-89; core/src/compact_remote.rs:384-385; core/tests/suite/compact.rs:2305-2369
- **Hypothesis:** Local, remote v1 and remote v2 pass active reasoning-summary setting, but collectors/replacement filtering ignore summary deltas/items.
- **Claimed impact:** Reasoning summary output tokens per compaction attempt, amplified by compaction retries.
- **Aggregation note:** Distinct from BUGS.md #31 Guardian-only unused summaries. Strong cross-compaction miss.

### H015. Guardian sampler output limit stops after first delta

- **Lane:** `reasoning-reviewers`
- **Agent confidence:** conditional
- **Waste class:** background,output
- **Evidence:** ext/guardian-v2/src/async_scorer/sampler.rs:653-685
- **Hypothesis:** Only first text delta is checked; classification returns while background drain ignores all later generated text until completion, with no provider output cap.
- **Claimed impact:** Normally one token but potentially provider-output ceiling across up to 16 concurrent classifiers.
- **Aggregation note:** Similar to first-pass candidate intentionally omitted from BUGS.md; still distinct. Likely low priority.

### H016. Remote v2 retained-message budget omits message framing

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** confirmed
- **Waste class:** cumulative,multiplicative,compaction-triggered
- **Evidence:** core/src/compact_remote_v2.rs:585-702; core/src/context_manager/history.rs:696-703,727-773; core/src/session/mod.rs:3496-3547; core/src/session/turn.rs:371-390; core/src/client.rs:885-990
- **Hypothesis:** Ordinary retained messages are charged only text tokens, minimum one, excluding item/role/content wrappers, IDs and metadata; replacement request uses fully serialized items.
- **Claimed impact:** One-character item costs one retained token but about 20 serialized tokens; about 13.6K tiny messages can approach 272K while consuming 13.6K of 64K budget.
- **Aggregation note:** Distinct from BUGS.md #16 audio zero cost and #3 general estimator omission. Strong.

### H017. Prior local compaction summaries survive v2 as real user messages

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** conditional
- **Waste class:** migration-triggered,cumulative,multiplicative
- **Evidence:** core/src/compact.rs:549-558,722-730; core/src/context/compaction_summary.rs:17-35; core/src/context/contextual_user_message.rs:18-43; core/src/event_mapping.rs:98-146,180-192; core/src/compact_remote_v2.rs:480-509
- **Hypothesis:** CompactionSummary is not recognized as contextual synthetic user content, so a local/legacy model-generated summary is retained as an ordinary user message alongside the new v2 compaction item.
- **Claimed impact:** Old summary can consume up to retained 64K budget and recur in downstream requests/compactions until evicted.
- **Aggregation note:** Distinct from BUGS.md #29 summary overlap with retained real user messages. Strong conditional.

### H018. Manual remote v2 compaction invokes model on pristine history

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** confirmed
- **Waste class:** per-invocation,cumulative
- **Evidence:** core/src/tasks/compact.rs:41-50; core/src/compact_remote_v2.rs:109-140,480-509; core/src/compact_remote_v2_attempt.rs:41-112; core/src/client.rs:579-580; core/tests/suite/rollout_budget.rs:334-405
- **Hypothesis:** No empty-history guard; manual compact sends base instructions, complete catalog and trigger, then retains meaningless compaction artifact. V1 has a no-op guard.
- **Claimed impact:** One full model call plus base/catalog input and retained artifact per pristine manual compact.
- **Aggregation note:** Related to but distinct from omitted BUGS.md first-pass local empty compact edge. Novel v2 variant; could merge with local empty compact when updating report.

### H019. HTTP SSE terminal errors can be erased by later framing failure

- **Lane:** `http-recovery`
- **Agent confidence:** conditional
- **Waste class:** retry-triggered,multiplicative
- **Evidence:** codex-api/src/sse/responses.rs:183-196,503-568,656-674,702-729,810-819; protocol/src/error.rs:380-415; core/src/session/turn.rs:1380-1459; model-provider-info/src/lib.rs:30,35,369-372
- **Hypothesis:** Standalone error events are unhandled; recognized terminal response.failed errors are held until clean EOF, so a later SSE framing failure or idle timeout replaces them with retryable generic stream error and replays full request.
- **Claimed impact:** Up to 6 full sampling requests by default, up to 101 configured; partial output can also repeat.
- **Aggregation note:** Distinct from BUGS.md #32 nested retry budgets and #10 WebSocket null-error classification. Strong conditional transport-state loss.

### H020. Resume resets the shared rollout budget ledger

- **Lane:** `context-reminders`
- **Agent confidence:** confirmed
- **Waste class:** resume-triggered,additional-calls
- **Evidence:** core/src/thread_manager.rs:1076-1085; core/src/agent/control.rs:145-160; core/src/rollout_budget.rs:35-40; core/src/session/session.rs:747-784; core/src/session/mod.rs:1350-1378
- **Hypothesis:** Resume preserves session/history/token usage but creates new AgentControl and RolloutBudget with weighted_tokens_used=0, reopening the full configured session allowance.
- **Claimed impact:** Up to limit_tokens additional weighted usage per resume; repeated resumes make effective cap unbounded.
- **Aggregation note:** Distinct from BUGS.md #30 reminder messages; this resets enforcement itself. Top novel finding.

### H021. Same-window resume re-arms reminder one-shot state

- **Lane:** `context-reminders`
- **Agent confidence:** confirmed
- **Waste class:** resume-triggered,cumulative
- **Evidence:** core/src/session/rollout_reconstruction.rs:378-384; core/src/session/mod.rs:1460-1522; core/src/session/time_reminder.rs:39-43,68-90; core/src/state/auto_compact_window.rs:44-56,72-74; core/src/session/token_budget.rs:90-125
- **Hypothesis:** History retains old reminders, but resume restores only window IDs and leaves time cadence/token reminder/fallback flags fresh, so same-window reminders fire again.
- **Claimed impact:** About 430 default heuristic tokens per resume, configured up to about 1000 plus time fragment; repeated resumes can trigger earlier compaction.
- **Aggregation note:** Distinct from BUGS.md #30 normal cadence accumulation. Strong.

### H022. Queued V2 completion results miss the next user turn first request

- **Lane:** `agent-status-paths`
- **Agent confidence:** confirmed
- **Waste class:** extra-call,cumulative
- **Evidence:** core/src/session/mod.rs:2119-2149; core/src/session/handlers.rs:81-95; core/src/session/turn_input.rs:413-420; core/src/session/turn.rs:267,307-314,413-425; core/tests/suite/subagent_notifications.rs:2585-2630
- **Hypothesis:** Queue-only completion mail does not wake idle thread and is deliberately withheld from first request of a new user turn, forcing a second full parent inference to deliver an already available result.
- **Claimed impact:** One avoidable full parent inference per relevant user turn with pending result mail.
- **Aggregation note:** Distinct from BUGS.md #23 repeated status bodies. Top novel.

### H023. V1 completion watcher is not rearmed for reusable agents

- **Lane:** `agent-status-paths`
- **Agent confidence:** confirmed
- **Waste class:** extra-call,polling-triggered
- **Evidence:** core/src/agent/control/spawn.rs:768-779,1231-1246; core/src/agent/control.rs:583-658; core/src/tools/handlers/multi_agents/send_input.rs:38-142; core/src/tools/handlers/multi_agents_spec.rs:147-170; core/src/session/mod.rs:1999-2030
- **Hypothesis:** Watcher exits after first final status; send_input reuses agent without new watcher, so subsequent completions are not forwarded and require explicit wait round trips.
- **Claimed impact:** At least one extra parent inference per reused V1 task whose result is needed.
- **Aggregation note:** Distinct from BUGS.md #23 wait result replay. Strong.

### H024. Completion truncation expands after JSON/XML rendering

- **Lane:** `agent-status-paths`
- **Agent confidence:** confirmed
- **Waste class:** per-completion,cumulative
- **Evidence:** core/src/session_prefix.rs:9-23; core/src/context/subagent_notification.rs:34-45; core/src/context_manager/history.rs:470-535; protocol/src/agent_path.rs:117-171; core/src/context/inter_agent_completion_message.rs:40-44; protocol/src/protocol.rs:861-890
- **Hypothesis:** Only raw payload is truncated to 900 heuristic tokens; quote/newline/NUL escaping and unbounded V2 agent paths are added afterward with no final rendered cap.
- **Claimed impact:** Quote-heavy output about 1832 tokens; NUL-heavy about 5432 tokens after nominal 900-token truncation, plus unbounded path fields.
- **Aggregation note:** Distinct from BUGS.md #23 replay frequency. Strong.

### H025. Nonterminal agent errors are published as terminal failures

- **Lane:** `agent-status-paths`
- **Agent confidence:** conditional
- **Waste class:** event-triggered,cumulative,polling-triggered
- **Evidence:** protocol/src/protocol.rs:1856-1877; core/src/agent/status.rs:6-20; core/src/session/mod.rs:2256-2260; core/src/session/handlers.rs:98-113; core/src/tasks/user_shell.rs:130-159,373-383
- **Hypothesis:** Every EventMsg::Error becomes terminal AgentStatus::Errored even when protocol says error does not affect turn status; V1 watcher injects false failure and exits before genuine completion.
- **Claimed impact:** One false model-visible result plus later explicit wait/recovery inference.
- **Aggregation note:** Distinct from BUGS.md #23 sticky final results. Occurrence depends on auxiliary error.

### H026. Hybrid Code Mode serializes schemas twice

- **Lane:** `code-mode`
- **Agent confidence:** confirmed
- **Waste class:** per-request,multiplicative
- **Evidence:** core/src/tools/spec_plan.rs:533-551; tools/src/code_mode.rs:8-43,114-160; core/src/client.rs:896-932; tools/src/code_mode_tests.rs:33-80
- **Hypothesis:** Each direct tool retains native JSON input/output schemas while equivalent TypeScript declaration is appended to description; both transmit on every request.
- **Claimed impact:** Up to duplicated schema size per eligible tool times turns/retries.
- **Aggregation note:** Distinct from BUGS.md #1 aggregate-unbounded CodeModeOnly catalog. First-pass candidate was omitted from BUGS; now novel relative to artifact.

### H027. Code Mode runtime state and model-visible handles are not checkpointed together

- **Lane:** `code-mode`
- **Agent confidence:** confirmed
- **Waste class:** compaction-triggered,resume-triggered,repeated-work
- **Evidence:** code-mode-runtime/src/session_runtime/mod.rs:39-71,156-168,273-290; core/src/session/rollout_reconstruction.rs:375-384; core/src/session/session.rs:1433-1437; code-mode/src/grpc_session/reconnect.rs:121-167; code-mode/src/grpc_session/generation.rs:57-67; core/src/session/turn.rs:460-499; core/tests/suite/code_mode.rs:1275-1305; code-mode-runtime/src/service.rs:389-398
- **Hypothesis:** Compaction can preserve live cell but remove its handle from model context; cold resume/reconnect preserves handles/store claims but creates empty runtime, causing useless wait/load and rerun of nested work.
- **Claimed impact:** At least one failed tool and corrective inference; potentially all nested work repeated.
- **Aggregation note:** Distinct from BUGS.md #12 stale notify and #25 output policy re-expansion. Strong.

### H028. Long-lived Code Mode cells dispatch through later StepContext

- **Lane:** `code-mode`
- **Agent confidence:** conditional
- **Waste class:** background,corrective-call,repeated-work
- **Evidence:** core/src/tools/code_mode/delegate.rs:27-78,101-183; core/src/session/turn.rs:1370-1379; core/src/tools/parallel.rs:41-45; core/tests/suite/code_mode.rs:3336-3422,3528-3623,4918-4935; core/src/tasks/mod.rs:909-919; core/src/tools/code_mode/mod.rs:146-156
- **Hypothesis:** Cells retain no originating turn/router; delayed callbacks use current request host after tool catalog changes, while interrupt may kill unrelated prior cells or leave them running when feature off.
- **Claimed impact:** Failed delayed nested call or destroyed background work can require rerun and additional model calls.
- **Aggregation note:** Distinct from BUGS.md #12 notification queue. Needs careful false-positive review.

### H029. Missing response item IDs defeat incremental WebSocket continuation

- **Lane:** `websocket-recovery`
- **Agent confidence:** conditional
- **Waste class:** per-call,cumulative,multiplicative
- **Evidence:** core/src/client.rs:2190-2225,387-399,1315-1352; core/src/session/turn.rs:2181-2193; core/src/session/mod.rs:3067-3128; core/tests/common/responses.rs:932-941; core/tests/suite/agent_websocket.rs:286-344
- **Hypothesis:** LastResponse stores raw item with id=None, later history persistence synthesizes an ID, and strict equality including ID fails, forcing next tool continuation to send full input without previous_response_id.
- **Claimed impact:** One active-context replay per affected tool round; repeated calls create triangular full-history input.
- **Aggregation note:** Distinct from BUGS.md #10 retry replay; this occurs on successful tool continuations. Provider must omit/empty IDs.

### H030. Service-tier changes force full replay and orphan startup prewarm

- **Lane:** `websocket-recovery`
- **Agent confidence:** likely
- **Waste class:** per-transition,cache-loss
- **Evidence:** core/src/client.rs:330-384; app-server-protocol/src/protocol/v2/turn.rs:222-226; core/tests/suite/agent_websocket.rs:372-470,483-547
- **Hypothesis:** Turn-local service_tier participates in continuation identity, so first-turn override cannot reuse generate=false prewarm and later tier transitions drop previous_response_id and resend full context.
- **Claimed impact:** Up to active context window per transition; first-turn override also leaves separate prewarm request.
- **Aggregation note:** Distinct from BUGS.md #19 tool catalog cache invalidation and #10 retry replay. Need determine whether server contract truly requires tier identity.

### H031. Pre-start MCP caches are exposed as live model catalogs

- **Lane:** `mcp-discovery`
- **Agent confidence:** confirmed
- **Waste class:** per-request,cumulative,multiplicative,extra-call,cache-loss
- **Evidence:** codex-mcp/src/connection_manager/tool_catalog.rs:180-236; codex-mcp/src/rmcp_client.rs:545-575; codex-mcp/src/tool_catalog_cache.rs:32-33,132-137; connectors/src/connector_runtime/mod.rs:187-223; core/tests/suite/mcp_tool_cache.rs:546-550,647-679
- **Hypothesis:** Cached definitions bypass startup waiting and enter inference as live tools; regular cache stays eligible 30m and Apps disk snapshots have no freshness check, so stale tools/descriptions can be advertised before server initialization.
- **Claimed impact:** Entire stale catalog per request; Apps cache accepts up to 32MiB. Stale callable can fail and require corrective model continuation.
- **Aggregation note:** Distinct from BUGS.md #1 size bounds and fixed MCP instruction refetch. Top novel.

### H032. MCP tool-list and recovered metadata changes are never adopted

- **Lane:** `mcp-discovery`
- **Agent confidence:** confirmed
- **Waste class:** per-request,cumulative,retry-triggered,extra-call,cache-loss
- **Evidence:** rmcp-client/src/logging_client_handler.rs:82-88; codex-mcp/src/rmcp_client.rs:112-124,962-968; rmcp-client/src/rmcp_client.rs:1283-1307,1428-1483; codex-mcp/src/connection_manager_tests.rs:4497-4623
- **Hypothesis:** tools/list_changed is only logged; ManagedClient freezes tools/instructions at startup, same-identity refresh reuses without relist, and HTTP recovery discards new peer metadata.
- **Claimed impact:** Stale schemas/instructions persist every request until forced reconnect; removed tools cause failed calls and extra inference.
- **Aggregation note:** Distinct from BUGS.md #19 catalog mutation cache loss; this suppresses legitimate updates and retains obsolete payload. Strong.

### H033. Plugin provenance sentence repeats in every MCP child tool

- **Lane:** `mcp-discovery`
- **Agent confidence:** confirmed
- **Waste class:** per-request,multiplicative
- **Evidence:** codex-mcp/src/rmcp_client.rs:669-733; codex-mcp/src/mcp/mod.rs:230-314; tools/src/mcp_tool.rs:36-54; core/src/tools/handlers/mcp.rs:450-483
- **Hypothesis:** Same namespace-level plugin display-name sentence is appended to every child description even though tools are grouped into one namespace.
- **Claimed impact:** About 10+ duplicate tokens per tool; roughly 1000 tokens/request for a 100-tool plugin before turns/retries/compaction.
- **Aggregation note:** Distinct from BUGS.md #1 aggregate catalog size. Direct repeated material.

### H034. Hidden MCP tools can rename visible tools before exposure filtering

- **Lane:** `mcp-discovery`
- **Agent confidence:** conditional
- **Waste class:** per-request,cache-loss
- **Evidence:** codex-mcp/src/connection_manager/tool_catalog.rs:43-58,283-315; codex-mcp/src/tools.rs:113-195; core/src/mcp_tool_exposure.rs:104-140
- **Hypothesis:** Sanitization/collision hashing happens before visibility filtering, so hidden colliders add suffixes to visible names and hidden-only churn can mutate visible declarations.
- **Claimed impact:** Extra identifier bytes each request; hidden churn can invalidate prompt prefix and leave obsolete callable names in history.
- **Aggregation note:** Distinct from BUGS.md #19 visible catalog mutation. Lower priority.

### H035. Local overflow trimming is discarded when replacement history is rebuilt

- **Lane:** `local-compaction`
- **Agent confidence:** confirmed
- **Waste class:** cumulative,multiplicative,retry-triggered
- **Evidence:** core/src/compact.rs:257-320,352-390,549-569,657-730; protocol/src/items.rs:483-500; core/src/session/turn.rs:460-499,1028-1055,1400-1425
- **Hypothesis:** Overflow trims a cloned history for successful compaction, then replacement re-clones original untrimmed session history. Removed real user messages return; content-only 20K budget ignores envelopes/item count/summary/context, and media-only turns become zero-cost empty retained messages.
- **Claimed impact:** 1000 empty envelopes about 31K tokens; 20K one-byte messages can serialize around 625K heuristic tokens, causing compaction followed by doomed request.
- **Aggregation note:** Distinct from BUGS.md #6 one-item retry; even ideal bulk trim is discarded. Top novel.

### H036. Local manual compaction runs on empty or unchanged history

- **Lane:** `local-compaction`
- **Agent confidence:** confirmed
- **Waste class:** extra-call,cumulative,repeated
- **Evidence:** core/src/session/handlers.rs:236-243; core/src/tasks/compact.rs:28-78; core/src/compact.rs:245-294,352-388; core/tests/suite/compact.rs:5068-5124
- **Hypothesis:** No semantic no-op preflight; summarization prompt invokes model even with no prior content, installs meaningless summary, and repeated compact calls repeat indefinitely.
- **Claimed impact:** One full compaction inference plus retained summary per invocation.
- **Aggregation note:** Not in BUGS.md because first-pass edge finding was omitted. Add low-ranked.

### H037. Compaction requests reasoning summaries and discards them

- **Lane:** `local-compaction`
- **Agent confidence:** confirmed
- **Waste class:** output,retry-triggered,multiplicative
- **Evidence:** core/src/compact.rs:735-795,352-359; core/src/session/turn_context.rs:251-260; core/src/client.rs:863-985; core/src/session/turn.rs:1257-1269; core/src/compact_remote_v2.rs:382-448; core/src/compact_remote_request.rs:86-89; core/src/compact_remote.rs:384-385
- **Hypothesis:** Local and remote compaction pass active reasoning.summary but collectors/replacement keep only assistant/compaction output and drop reasoning summaries.
- **Claimed impact:** One unused plaintext reasoning summary per compact attempt, often tens to hundreds of tokens and more under detailed.
- **Aggregation note:** Distinct from BUGS.md #31 Guardian unused summaries. Merge with reviewer lane duplicate candidate.

### H038. Small or explicitly selected MCP catalogs are deferred unconditionally

- **Lane:** `deferred-tool-search`
- **Agent confidence:** conditional
- **Waste class:** extra-call,multiplicative
- **Evidence:** core/src/mcp_tool_exposure.rs:84-98; core/src/tools/spec_plan.rs:347-381,496-530; core/tests/suite/search_tool.rs:209-247,509-839
- **Hypothesis:** Whenever search is supported, all eligible tools are deferred regardless of catalog size or explicit app mention; first use requires search, tool call and final response instead of direct tool call and final response.
- **Claimed impact:** At least one extra full sampling invocation for first use; net waste when catalog is small/already selected.
- **Aggregation note:** Distinct from BUGS.md #7 duplicate search results. Strong conditional threshold issue.

### H039. Compaction pays for discovery schemas it always discards

- **Lane:** `deferred-tool-search`
- **Agent confidence:** likely
- **Waste class:** per-compaction,multiplicative
- **Evidence:** core/src/compact.rs:257-283,354-359,644-732; core/src/compact_remote_request.rs:33-65; core/src/compact_remote_v2_attempt.rs:41-74; core/src/compact_remote.rs:370-454; core/src/compact_remote_v2.rs:493-500
- **Hypothesis:** Local/v1/v2 compaction submit ToolSearchCall/Output history, including full schemas, but installed compacted histories always remove them; trimming only acts if full context already exceeds window.
- **Claimed impact:** Up to about 8.2K tokens per maximum search output per attempt, multiplied by retries/fallback.
- **Aggregation note:** Distinct from BUGS.md #17 current tool catalog; this is historical discovery output. Strong.

### H040. Oversized first search result suppresses valid later matches

- **Lane:** `deferred-tool-search`
- **Agent confidence:** conditional
- **Waste class:** extra-call,repeated-search
- **Evidence:** core/src/tools/handlers/tool_search.rs:237-258; tools/src/responses_api.rs:90-112; tools/src/tool_discovery.rs:48-64; core/src/tools/context.rs:192-198; tools/src/tool_discovery_tests.rs:116-126
- **Hypothesis:** Limiter stops at first non-fitting namespace child rather than skipping it; output still says completed with no truncation/continuation marker, so later smaller matches disappear and model must search again.
- **Claimed impact:** At least one extra search/follow-up; can repeat with same ranking.
- **Aggregation note:** Distinct from BUGS.md #7 duplicate accumulation/compaction loss. Novel.

### H041. Cold resume turns interrupted search into false successful empty result

- **Lane:** `deferred-tool-search`
- **Agent confidence:** confirmed
- **Waste class:** resume-triggered,cumulative,extra-call
- **Evidence:** core/src/stream_events_utils.rs:296-324; core/src/session/turn.rs:2155-2165; core/src/session/rollout_reconstruction.rs:378-384; core/src/context_manager/normalize.rs:69-85; core/src/context_manager/history_tests.rs:2031-2065
- **Hypothesis:** Crash after persisted call but before output leaves orphan; normalization synthesizes completed client search with tools=[] rather than aborted/replaying deterministic search.
- **Claimed impact:** One additional search and follow-up inference per interrupted search; synthetic empty result regenerates until compaction.
- **Aggregation note:** Distinct from BUGS.md #7 compaction-before-consumption. Strong.

### H042. Memory extraction reuploads all historical discovery schemas across compaction

- **Lane:** `deferred-tool-search`
- **Agent confidence:** confirmed
- **Waste class:** background,cumulative
- **Evidence:** rollout/src/policy.rs:66-77; memories/write/src/phase1.rs:286-303,404-440; memories/write/src/prompts.rs:98-118; features/src/lib.rs:1036-1040
- **Hypothesis:** Phase1 serializes raw rollout ToolSearchCall/Output verbatim and ignores compacted checkpoints instead of applying replacement history, so schemas removed from active context are sent again to memory model.
- **Claimed impact:** Can consume most of 70 percent extraction-model input allowance and displace conversation; feature default-disabled.
- **Aggregation note:** Distinct from BUGS.md #7 active history and #14 Phase2 retries. High conditional impact.

### H043. Realtime override paths bypass aggregate instruction limits

- **Lane:** `realtime`
- **Agent confidence:** confirmed
- **Waste class:** per-session,multiplicative
- **Evidence:** app-server-protocol/src/protocol/v2/realtime.rs:197-254; config/src/config_toml.rs:407-418; core/src/realtime_prompt.rs:5-21; core/src/realtime_conversation.rs:1315-1418; core/src/session/world_state.rs:136-149; core/src/context/realtime_start_with_instructions.rs:39-40
- **Hypothesis:** Only start/end instructions get 8192-token validation; request prompt and configured backend/startup/ordinary-backend instructions copy verbatim into model-visible context.
- **Claimed impact:** No code ceiling; 1MiB is about 262K heuristic tokens and can exceed context.
- **Aggregation note:** Related first-pass candidate was omitted from BUGS as configuration-driven; distinct from BUGS #27 duplicated handoff. Need strict avoidability review.

### H044. Frameless realtime reconnect replays already-sent context frames

- **Lane:** `realtime`
- **Agent confidence:** confirmed
- **Waste class:** retry-triggered,multiplicative
- **Evidence:** codex-api/src/endpoint/realtime_websocket/methods_frameless_bidi.rs:11,109-123; codex-api/src/endpoint/realtime_websocket/methods.rs:441-466; core/src/realtime_conversation.rs:1856-1866,1895-1929,1964-1978; core/src/realtime_conversation/sideband.rs:49,131-164
- **Hypothesis:** Context appends split into 500-byte frames; failure after early frames retains original full text and reconnect restarts at frame one, with no attempt ceiling while active.
- **Claimed impact:** For capped 1K-token item late failure can duplicate about 875 tokens; uncapped live text append makes upper bound unlimited.
- **Aggregation note:** Distinct from BUGS #10 ordinary Responses WebSocket replay. Strong.

### H045. Transcript-tail flush invokes ordinary model for assistant-only speech

- **Lane:** `realtime`
- **Agent confidence:** confirmed
- **Waste class:** extra-call,background
- **Evidence:** codex-api/src/endpoint/realtime_websocket/methods.rs:609-636,661-664,1317-1365; core/src/realtime_conversation.rs:1788-1795,1997-2011; core/src/session/turn_input.rs:490-506
- **Hypothesis:** Any nonempty tail, including assistant-only transcript, is wrapped with acknowledgement instruction and routed through full coding-agent inference.
- **Claimed impact:** One unnecessary full ordinary inference per enabled realtime shutdown with assistant-only tail.
- **Aggregation note:** First pass found broader opt-in tail call but BUGS omitted it; this proves unnecessary assistant-only case. Strong when feature enabled.

### H046. Replacing realtime sessions can leave old model sessions running

- **Lane:** `realtime`
- **Agent confidence:** likely
- **Waste class:** background,multiplicative
- **Evidence:** codex-api/src/endpoint/realtime_websocket/methods.rs:411-429; core/src/realtime_conversation.rs:528-542,1095-1110,1610-1621,1871; app-server-protocol/src/protocol/v2/realtime.rs:279-284
- **Hypothesis:** Core cancels/drops transport without calling frameless session.close; replacement startup clears active flag so old closure notification is suppressed, while loss of control socket need not close media session.
- **Claimed impact:** Old WebRTC model session may continue transcription/generation until peer or service timeout; repeated replacements can accumulate.
- **Aggregation note:** Distinct from BUGS #27 handoff duplication. Depends on client peer cleanup/service timeout.

### H047. Residency eviction can drop nested-child completions

- **Lane:** `subagent-forwarding`
- **Agent confidence:** conditional
- **Waste class:** background,retry-triggered,extra-call
- **Evidence:** core/src/agent/control/residency.rs:123-155,233-239; core/src/agent/control/spawn.rs:540-543; core/src/session/mod.rs:2119-2154; core/src/thread_manager.rs:1519-1538
- **Hypothesis:** Idle nested parent can be evicted while descendants run; completion delivery requires parent loaded and failure is only logged, with no reload/durable queue/retry.
- **Claimed impact:** Lost automatic handoff requires status/list/follow-up work and may require another child inference.
- **Aggregation note:** Distinct from BUGS.md #23 status replay. Confirmed path under V2 residency pressure.

### H048. Legacy forks inherit stale parent token usage after filtering

- **Lane:** `subagent-forwarding`
- **Agent confidence:** confirmed
- **Waste class:** background,compaction-triggered,multiplicative
- **Evidence:** core/src/agent/control/spawn.rs:63-100,970-980; core/src/session/mod.rs:1388-1399,1558-1562; core/src/context_manager/history.rs:416-450; core/src/session/turn.rs:1027-1055; protocol/src/protocol.rs:753-759
- **Hypothesis:** Legacy fork filters out much history but retains parent TokenCount events; child installs usage unchanged and can compact before its first inference.
- **Claimed impact:** One unnecessary full compaction call per affected child; N forks multiply.
- **Aggregation note:** Distinct from BUGS.md #11 duplicated initial context. Paginated forks strip token events. Strong.

### H049. Peer follow-up results are delivered to direct parent rather than requester

- **Lane:** `subagent-forwarding`
- **Agent confidence:** likely
- **Waste class:** background,cumulative,extra-call
- **Evidence:** core/src/tasks/mod.rs:454-508; core/src/session/mod.rs:2073-2154; core/tests/suite/subagent_notifications.rs:2334-2630
- **Hypothesis:** Initiating peer gets only UI activity; model-visible result enters target direct parent context, so requester must poll or receive relay.
- **Claimed impact:** Up to about 1K result tokens in wrong context plus parent/status/requester relay inferences.
- **Aggregation note:** Distinct from BUGS.md #23 status bodies and #11 forks. Depends on intended integration ownership.

### H050. MCP numeric argument bounds are stripped before model serialization

- **Lane:** `mcp-tool-schemas`
- **Agent confidence:** conditional
- **Waste class:** retry-triggered,extra-call
- **Evidence:** tools/src/json_schema.rs:54-56,136-140,548-556; tools/src/json_schema_tests.rs:230-259; tools/src/mcp_tool.rs:19-55; tools/src/responses_api.rs:31-44,120-127
- **Hypothesis:** External schema normalization removes minimum/maximum, advertising bounded integers as unrestricted; no equivalent local range validation occurs before server call.
- **Claimed impact:** Invalid range call yields failed tool output and at least one full follow-up inference.
- **Aggregation note:** Distinct from BUGS.md #1 catalog size. Common count/limit/page args exposed.

### H051. Goal continuations append the full 6.3K-character policy every time

- **Lane:** `goals`
- **Agent confidence:** confirmed
- **Waste class:** cumulative,multiplicative,resume-triggered
- **Evidence:** ext/goal/src/steering.rs:9-72,120-150; ext/goal/templates/goals/continuation.md; ext/goal/src/runtime.rs:363-417; core/src/hook_runtime.rs:656-658; core/src/session/mod.rs:3175-3222; core/src/context_manager/history.rs:188-204; core/src/session/turn.rs:371-390
- **Hypothesis:** Every continuation persists full static policy, objective and counters; prior copies survive pause/clear/terminal and resume. XML escaping can further expand objective.
- **Claimed impact:** Ten continuations retain about 63K static chars and cumulatively submit about 346K; 4000 ampersands expand objective to 26241 bytes.
- **Aggregation note:** BUGS.md #4 covered polling but omitted repeated continuation payload. Top novel.

### H052. Root goal budget ignores descendant model usage

- **Lane:** `goals`
- **Agent confidence:** confirmed
- **Waste class:** background,multiplicative,budget-bypass
- **Evidence:** ext/goal/src/extension.rs:96-128,330-354; core/src/session/mod.rs:4094-4105; core/src/agent/control/spawn.rs:630-639,812-819; protocol/src/protocol.rs:2869-2879,2912-2920
- **Hypothesis:** Goal accounting is thread-local and token callbacks charge only current thread, so subagent/non-root model usage is absent from root budget except small returned result.
- **Claimed impact:** Multiple workers can consume arbitrary multiples of configured root budget while goal remains active.
- **Aggregation note:** Distinct from BUGS.md #11 fork context and #26 interrupt. Strong.

### H053. Pause clear and goal replacement do not stop active inference

- **Lane:** `goals`
- **Agent confidence:** confirmed
- **Waste class:** background,extra-call,budget-misattribution
- **Evidence:** tui/src/slash_command.rs:208-264; tui/src/chatwidget/slash_dispatch.rs:840-895; tui/src/app/thread_goal_actions.rs:150-295; ext/goal/src/api.rs:285-329; ext/goal/src/runtime.rs:145-238
- **Hypothesis:** Goal state mutates while turn continues; pause/clear neither interrupt nor inject stop, and replacement can let active turn pursue old objective while usage charges to new goal.
- **Claimed impact:** Remainder of current generation plus potentially multiple tool-follow-up model calls after user paused/cleared/replaced goal.
- **Aggregation note:** Distinct from BUGS.md #26 interrupt relaunch because this path never interrupts. Strong.

### H054. Goal control tools echo full objective and metadata

- **Lane:** `goals`
- **Agent confidence:** confirmed
- **Waste class:** per-call,cumulative
- **Evidence:** ext/goal/src/tool.rs:50-67,170-308,433-523; protocol/src/protocol.rs:3939-3966; core/src/session/turn.rs:2155-2168
- **Hypothesis:** get/create/update return complete ThreadGoal including objective already in call/context plus IDs/timestamps/derivable remainingTokens; JSON persists in history.
- **Claimed impact:** About 4256 serialized bytes for max ASCII objective per call, larger with multibyte content.
- **Aggregation note:** Distinct from BUGS.md #4 polling and #26 interrupt. Direct duplicate material.

### H055. Goal accounting failures fail open into further model turns

- **Lane:** `goals`
- **Agent confidence:** conditional
- **Waste class:** retry-triggered,extra-call,budget-bypass
- **Evidence:** ext/goal/src/extension.rs:235-323,360-405; ext/goal/src/runtime.rs:363-417,454-510; core/src/tasks/mod.rs:824-857; core/src/tasks/lifecycle.rs:25-62
- **Hypothesis:** Tool/turn accounting write errors are logged and swallowed; stored goal can remain active and idle continuation starts another full turn without enforcing budget.
- **Claimed impact:** One extra continuation after transient boundary failure; potentially unbounded with persistent read-success/write-fail.
- **Aggregation note:** Distinct from known broken-tool retry and BUGS.md #26. Failure-mode finding.

### H056. Standalone web search resends two uncapped user messages

- **Lane:** `misc-model-calls`
- **Agent confidence:** confirmed
- **Waste class:** per-request,multiplicative
- **Evidence:** ext/web-search/src/history.rs:24-25; ext/web-search/src/tool.rs:116-145; protocol/src/user_input.rs:9; ext/web-search/src/output.rs:30
- **Hypothesis:** Separate alpha-search model receives last two full user messages after coding model already processed them; only assistant tail is capped.
- **Claimed impact:** Up to about two million chars, roughly 524K heuristic tokens in builder; practical current prompt can approach 262K duplicated tokens.
- **Aggregation note:** First pass candidate omitted from BUGS.md; distinct from catalog/request findings. Strong.

### H057. Title and recap workers inherit discarded reasoning settings

- **Lane:** `misc-model-calls`
- **Agent confidence:** conditional
- **Waste class:** background,output,retry-triggered
- **Evidence:** tui/src/app/thread_title.rs:53,55; tui/src/app/recap.rs:250,368; tui/src/temporary_structured_request.rs:59,201; app-server/src/request_processors/turn_processor.rs:866; core/src/session/step_settings.rs:58,79; core/src/client.rs:195,865
- **Hypothesis:** Fallback titles and recaps preserve configured/default reasoning effort and summaries; worker consumes only final AgentMessage and discards hidden reasoning.
- **Claimed impact:** Potentially hundreds or thousands of hidden reasoning tokens for a 36-char title or 320-char recap.
- **Aggregation note:** Distinct from BUGS.md #5 oversized input context. Strong when high/max reasoning configured.

### H058. Single MCP result bypasses output cap through per-item framing

- **Lane:** `mcp-results-resources`
- **Agent confidence:** confirmed
- **Waste class:** per-call,cumulative,compaction-triggered
- **Evidence:** protocol/src/models.rs:2243-2379; core/src/tools/context.rs:146-173; utils/output-truncation/src/lib.rs:94-212; core/src/context_manager/history.rs:673-770
- **Hypothesis:** Truncation charges payload/media cost but not each content item JSON framing and has no item-count cap; empty encrypted blocks cost zero and adjacent texts are not coalesced.
- **Claimed impact:** About 12K one-byte blocks serialize around 384KiB, about 96K tokens; empty encrypted blocks code-unbounded.
- **Aggregation note:** Distinct from BUGS.md #2 multiplication across separate outputs. Top novel.

### H059. MCP resource truncation has no progressive continuation

- **Lane:** `mcp-results-resources`
- **Agent confidence:** conditional
- **Waste class:** extra-call,retry-triggered,cumulative
- **Evidence:** core/src/tools/handlers/mcp_resource.rs:142-185,346-362; core/src/tools/handlers/mcp_resource/read_mcp_resource.rs:44-96; codex-mcp/src/binding_clients.rs:80-159; utils/output-truncation/src/lib.rs:30-36
- **Hypothesis:** All-server listings flatten all pages, discard cursors and middle-truncate; oversized reads expose deterministic head/tail with no range or continuation, so repeat yields same bytes.
- **Claimed impact:** Futile repeat adds another roughly 12K-token result and inference; all-server recovery requires fan-out.
- **Aggregation note:** Distinct from BUGS.md #24 binary JSON textification. Strong conditional.

### H060. MCP resource cursors bypass bounds and repeat in call and result

- **Lane:** `mcp-results-resources`
- **Agent confidence:** conditional
- **Waste class:** output,cumulative,retry-triggered,loop
- **Evidence:** core/src/tools/handlers/mcp_resource.rs:49-78,142-178,326-336; codex-mcp/src/binding.rs:104-135; codex-mcp/src/pagination.rs:9-74; protocol/src/models.rs:1037-1095
- **Hypothesis:** Single-server pagination bypasses 64KiB cursor, repeated-cursor and page guards; cursor appears in result then model-generated next call and both remain in history.
- **Claimed impact:** 40KiB cursor costs about 10K result tokens plus about 10K output tokens in next call; repeated cursor can loop unbounded.
- **Aggregation note:** Distinct from BUGS.md #24 content conversion. Strong if server emits large/repeated cursor.

### H061. MCP resource tools bypass memory external-context suppression

- **Lane:** `mcp-results-resources`
- **Agent confidence:** conditional
- **Waste class:** background,cumulative
- **Evidence:** core/src/tools/handlers/mcp_resource.rs:280-324; tools/src/tool_output.rs; core/src/tools/registry.rs:773-788; core/src/stream_events_utils.rs:132-157; core/src/mcp_tool_call.rs:834-850; codex-mcp/src/server.rs:409-415; state/src/runtime/memories.rs:148-268; memories/write/src/phase1.rs:283-323
- **Hypothesis:** Resource tools leave contains_external_context=false, unlike regular MCP; thread remains memory-eligible and later background extraction includes retained external resource output.
- **Claimed impact:** At least one phase1 model call containing resource output, plus possible phase2 consolidation.
- **Aggregation note:** Distinct from BUGS.md #14 Phase2 retry and max automation-memory finding. Requires MemoryTool and suppression enabled.

### H062. Resource listings repeat server identity per descriptor

- **Lane:** `mcp-results-resources`
- **Agent confidence:** confirmed
- **Waste class:** per-call,cumulative
- **Evidence:** core/src/tools/handlers/mcp_resource.rs:110-178
- **Hypothesis:** Server already appears in call args and top-level response, yet is repeated in every resource/template entry.
- **Claimed impact:** Short srv field about 7680 tokens for 2048 entries; my-server about 47KiB, nearly common allowance.
- **Aggregation note:** Distinct from BUGS.md #24 wrapper/binary textification. Direct repeated material.

### H063. MCP audience and priority annotations are ignored

- **Lane:** `mcp-results-resources`
- **Agent confidence:** conditional
- **Waste class:** per-call,cumulative,extra-call
- **Evidence:** codex-mcp/src/binding.rs:365-381; protocol/src/models.rs:2285-2379; utils/output-truncation/src/lib.rs:94-212
- **Hypothesis:** Converter drops annotations; user-only content is sent to assistant and low-priority early blocks can consume budget before high-priority later blocks.
- **Claimed impact:** Up to complete output allowance unnecessary or useful content displaced, requiring another call.
- **Aggregation note:** Distinct from BUGS.md #24 unknown resource JSON fallback. Protocol-dependent.

### H064. MCP media is destructively stripped before model-dependent projection

- **Lane:** `mcp-results-resources`
- **Agent confidence:** conditional
- **Waste class:** model-switch,resume-triggered,extra-call
- **Evidence:** core/src/mcp_tool_call.rs:853-889; core/src/context_manager/normalize.rs:330-425
- **Hypothesis:** Result media is replaced based on current model before persistence, despite history supporting per-request modality projection; later capable model cannot recover and must rerun MCP call.
- **Claimed impact:** One extra inference and MCP execution per needed media result.
- **Aggregation note:** Distinct from BUGS.md #16 compaction audio and #24 resource textification. Strong lifecycle issue.

### H065. Every MCP result adds persistent wall-time text

- **Lane:** `mcp-results-resources`
- **Agent confidence:** likely
- **Waste class:** per-call,cumulative
- **Evidence:** core/src/tools/context.rs:146-173
- **Hypothesis:** Wall time and Output header prepend every result including empty, persist, and resend on later requests.
- **Claimed impact:** About 9-18 tokens each; 100 calls retain 1-2K active and can contribute 50-90K cumulative input.
- **Aggregation note:** Distinct from BUGS.md #24 resource conversion. Small but frequent.

### H066. Request shrink is not credited before compaction

- **Lane:** `context-estimation`
- **Agent confidence:** confirmed
- **Waste class:** compaction-triggered
- **Evidence:** core/src/context_manager/history.rs:434-449,207-223; core/src/session/turn.rs:159-177,1033-1058,1383-1407; core/src/context_manager/normalize.rs:328-417; core/tests/suite/model_switching.rs:865-976
- **Hypothesis:** Pre-turn decision carries previous server total before next-request normalization removes unsupported media/generated bytes or other request material, so it can compact even though actual next request already fits.
- **Claimed impact:** One unnecessary full compaction invocation, potentially processing nearly prior usable window.
- **Aggregation note:** Inverse of BUGS.md #3/#15 underestimation. Strong conditional on shrink crossing threshold.

### H067. Image token estimates mismatch GPT-5.6 accounting

- **Lane:** `context-estimation`
- **Agent confidence:** confirmed
- **Waste class:** compaction-triggered,retry-triggered
- **Evidence:** core/src/context_manager/history.rs:673-713,780-863; core/src/image_preparation_tests.rs:37-68,106-132,171-205; core/src/context_manager/history_tests.rs:2698-2766; core/src/client_common.rs:55-64; models-manager/models.json:4-31
- **Hypothesis:** Non-original images use fixed 1844 tokens regardless of dimensions; original uses raw patch count without 1.2 multiplier. Small images overestimate and large/original underestimate, driving premature compaction or failed admission.
- **Claimed impact:** 64x32: +1841 estimate; 1600x1600: -1156; 2304x864 original: -389; cap: -2000 per image.
- **Aggregation note:** Distinct from BUGS.md #15 media omitted entirely and #16 audio zero. Verify against current official image docs before inclusion.

### H068. Dynamic tool schemas silently lose runtime constraints

- **Lane:** `dynamic-tools`
- **Agent confidence:** likely
- **Waste class:** retry-triggered,cumulative
- **Evidence:** tools/src/json_schema.rs:41-77,200-203,477-556; app-server/src/request_processors/thread_processor.rs:363-367; tools/src/dynamic_tool.rs:5-14; core/src/tools/handlers/dynamic.rs:51-70; tui/src/dynamic_tools.rs:146-220,282-300,426-496,830-839; core/src/stream_events_utils.rs:297-327
- **Hypothesis:** Model-visible lowering drops string/array/numeric bounds and can change boolean schemas, while handler enforces original constraints; model-valid advertised args fail at execution and require corrective inference.
- **Claimed impact:** One failed tool output plus full-context follow-up per corrected call; repeated corrections multiply.
- **Aggregation note:** Generalizes max MCP numeric bound candidate; distinct from BUGS.md #1 size. Merge schema-semantic-loss root.

### H069. Thread-salted prefix IDs defeat shared prompt-cache keys

- **Lane:** `prompt-cache`
- **Agent confidence:** likely
- **Waste class:** cache-loss,multiplicative
- **Evidence:** core/src/client.rs:514-526,894-923; core/src/client_tests.rs:340-390; core/tests/suite/prompt_cache_key.rs:40-139; core/src/guardian/review_session.rs:298-308; core/src/guardian/tests.rs:3421-3428
- **Hypothesis:** Root/child and Guardian forks deliberately share cache routing keys, but identical Lite tool/base-instruction items receive thread-namespaced IDs, so rendered prefixes differ and cannot share cache entry.
- **Claimed impact:** Each P-token prefix needs independent entry; GPT-5.6 missed read becoming write can add up to 1.15x P input-price equivalents versus read.
- **Aggregation note:** Distinct from BUGS.md #19 catalog mutation and #20 compaction breakpoint. Provider semantics confirmed.

### H070. Parallel auto-review replays pending calls quadratically

- **Lane:** `parallel-tools`
- **Agent confidence:** confirmed
- **Waste class:** background,multiplicative
- **Evidence:** core/src/stream_events_utils.rs:316-324; core/src/guardian/prompt.rs:142-162,322-325,481-500,525-592; core/src/guardian/mod.rs:69-75; core/src/guardian/review_session.rs:618-628,768-815; core/src/guardian/tests.rs:3206-3456; core/src/guardian/review.rs:78,486-493,1042-1075
- **Hypothesis:** Each approval review clones live history containing earlier pending calls and duplicates current action as planned JSON; concurrent reviews use separate ephemeral sessions, retries multiply.
- **Claimed impact:** Ten 1K-token calls expose at least 55K transcript tokens versus 10K unique; theoretical 1024 concurrent Code Mode callbacks and up to 3 attempts each.
- **Aggregation note:** Distinct from BUGS.md #31 unused reasoning summaries and max Guardian action duplicate. Top novel.

### H071. Parallel hook context splices completed response and disables WebSocket continuation

- **Lane:** `parallel-tools`
- **Agent confidence:** conditional
- **Waste class:** cache-loss,per-wave,multiplicative
- **Evidence:** core/src/tools/parallel.rs:145-175; core/tests/suite/tool_parallelism.rs:304-428; core/src/hook_runtime.rs:177-210,764-774; core/src/tools/registry.rs:685-704; core/src/client.rs:1313-1352,1833-1854,2167-2224; core/tests/suite/client_websockets.rs:2181-2260
- **Hypothesis:** Hooks from early tool calls can append between server-returned call items; local history no longer equals completed response prefix, so previous_response_id reuse fails and follow-up sends full context.
- **Claimed impact:** Tiny hook message can force full active-context replay each affected wave.
- **Aggregation note:** Distinct from BUGS.md #8 hook size and #10 retry replay. Strong serialization path.

### H072. Release builds swallow fatal tool futures and still follow up

- **Lane:** `parallel-tools`
- **Agent confidence:** confirmed
- **Waste class:** extra-call,full-context
- **Evidence:** core/src/tools/parallel.rs:73-86; protocol/src/error.rs:380-407; core/src/session/turn.rs:2160-2178,2421-2426; core/src/util.rs:93-98; core/src/context_manager/normalize.rs:21-136; core/src/context_manager/history_tests.rs:1661
- **Hypothesis:** Fatal handler/task failure is logged during aggregate drain but success returned; needs_follow_up remains true, normalization fabricates aborted output, and model is called despite no useful result.
- **Claimed impact:** One unnecessary model invocation per fatal batch, often full context; synthetic abort persists in normalized prompts.
- **Aggregation note:** Distinct from BUGS.md transport retry findings. Release-only behavior.

### H073. Legacy forks compact from stale parent token usage

- **Lane:** `rollout-resume`
- **Agent confidence:** confirmed
- **Waste class:** background,compaction-triggered,multiplicative
- **Evidence:** core/src/agent/control/spawn.rs:63-99,970-981; core/src/session/mod.rs:1394-1398,1558-1562; core/src/session/context_window.rs:23-31; core/src/session/turn.rs:1028-1054
- **Hypothesis:** Legacy fork filters substantial parent history but retains parent TokenCount event; child restores it unchanged and may compact before first inference.
- **Claimed impact:** One unnecessary full compaction per affected child.
- **Aggregation note:** Duplicate of max subagent finding; distinct from BUGS.md #11. Merge.

### H074. Compound rollout writes expose half-committed model state

- **Lane:** `rollout-resume`
- **Agent confidence:** conditional
- **Waste class:** failure-triggered,cumulative,extra-call
- **Evidence:** core/src/session/mod.rs:4039-4055,3533-3542,3867-3872; thread-store/src/local/live_writer.rs:354-364; core/src/compact.rs:735-806; core/src/tasks/compact.rs:79-84; core/src/tasks/mod.rs:945-995; core/src/thread_manager.rs:2205-2254,2298-2334
- **Hypothesis:** Context/WorldState/TurnContext and compaction checkpoints persist as separate independently flushed appends; persistence/stream/interleaving gaps can leave context without baseline, partial failed compact outputs, or duplicate abort markers.
- **Claimed impact:** From one small duplicate marker to full repeated initial context or another compaction call.
- **Aggregation note:** Distinct from BUGS.md #11 normal full-history filtering. Three related half-commit windows; may split after verification.

### H075. Last-N forks retain context fragments after dropping their baseline

- **Lane:** `rollout-resume`
- **Agent confidence:** confirmed
- **Waste class:** background,cumulative
- **Evidence:** core/src/thread_rollout_truncation.rs:257-278; core/src/agent/control/spawn.rs:63-99,877-1017; core/src/session/mod.rs:3999-4005; core/src/context_manager/history.rs:459-474
- **Hypothesis:** Last-N keeps raw suffix user-role context/replacement history but intentionally drops TurnContext/WorldState baseline; first child turn injects full current context again.
- **Claimed impact:** One repeated current-state or near-full initial-context bundle per bounded child.
- **Aggregation note:** Distinct from BUGS.md #11 FullHistory baseline bug. Strong.

### H076. Standalone token-budget compaction baseline is ignored on reconstruction

- **Lane:** `rollout-resume`
- **Agent confidence:** confirmed
- **Waste class:** resume-triggered,cumulative
- **Evidence:** core/src/compact_token_budget.rs:26-44,79-84; core/src/session/mod.rs:3918-3965; core/src/session/rollout_reconstruction.rs:121-135,257-280
- **Hypothesis:** Standalone compaction persists full initial context and TurnContext without user boundary; reconstruction records Latest but promotes only user-turn Latest or Cleared, then resume/fork injects full bundle again.
- **Claimed impact:** Up to one full initial-context duplicate per affected resume/fork.
- **Aggregation note:** Distinct from BUGS.md #11 and applies default-disabled TokenBudget. Strong conditional.

### H077. Four-byte token heuristic can vastly exceed nominal output tokens

- **Lane:** `tool-output-retention`
- **Agent confidence:** conditional
- **Waste class:** per-output,cumulative,multiplicative
- **Evidence:** utils/string/src/truncate.rs:15,71; core/src/context_manager/history.rs:479
- **Hypothesis:** Token truncation budgets four UTF-8 bytes per token and then applies 1.2 allowance; token-dense code/punctuation/Unicode/opaque data can tokenize much denser than heuristic.
- **Claimed impact:** Nominal 10K policy can retain about 48KB and approach 48K actual tokens in worst content.
- **Aggregation note:** Distinct from BUGS.md #2 known 1.2 multiplier; this is tokenizer-density error. Provider tokenizer/content dependent.

### H078. Structured output framing and audio fallback defeat one-output budget

- **Lane:** `tool-output-retention`
- **Agent confidence:** confirmed
- **Waste class:** per-output,cumulative,compaction-triggered
- **Evidence:** utils/output-truncation/src/lib.rs:94-212; protocol/src/models.rs:2285-2379; utils/audio/src/lib.rs:200; core/src/context_manager/normalize.rs:381; core/src/context_manager/history.rs:727
- **Hypothesis:** Budget charges payload/media only, no item count/framing; zero-duration audio costs zero, then unsupported-audio normalization expands every retained item into text placeholders without rebudgeting.
- **Claimed impact:** 12K one-char blocks about 99K tokens; 12K audio placeholders about 276K heuristic tokens; zero-duration clips count-unbounded.
- **Aggregation note:** Merge max MCP result framing candidate; distinct from BUGS.md #2 cross-output and #16 compaction audio. Top novel.

### H079. Tool output limit can be disabled by configuration or signed wrapping

- **Lane:** `tool-output-retention`
- **Agent confidence:** confirmed
- **Waste class:** configuration-triggered,cumulative,multiplicative
- **Evidence:** config/src/config_toml.rs:305; models-manager/src/model_info.rs:39; protocol/src/protocol.rs:3220; core/src/config/mod.rs:2026; core/tests/suite/truncation.rs:50
- **Hypothesis:** tool_output_token_limit has no maximum; signed catalog i64 converts with unchecked as usize, so negative value becomes huge and effectively disables truncation.
- **Claimed impact:** Tests retain about 400KB at 100K config; negative/max values can make output effectively unbounded.
- **Aggregation note:** Distinct from BUGS.md #2 default multiplier/aggregate. Report negative wrapping strongly; user-selected high limit is intentional.

### H080. Resize notices survive for images later removed from tool output

- **Lane:** `tool-output-retention`
- **Agent confidence:** confirmed
- **Waste class:** per-output,cumulative
- **Evidence:** core/src/image_preparation.rs:119; utils/output-truncation/src/lib.rs:13; core/src/context/image_resize_notice.rs:56; core/src/context_manager/history.rs:525; features/src/lib.rs:1369
- **Hypothesis:** Preparation emits notice for every resized output image before four-image/byte/token truncation; complete developer notice persists even when most images are dropped.
- **Claimed impact:** Potentially many stale lines per multimodal result until compaction.
- **Aggregation note:** Distinct from known old-image retention and BUGS.md #16 audio. Feature default-disabled.

### H081. Rolled-back compaction mutates replay state before survival is known

- **Lane:** `history-normalization`
- **Agent confidence:** conditional
- **Waste class:** resume-triggered,rework,cumulative
- **Evidence:** core/src/session/rollout_reconstruction.rs:189-222,93-115,373-454
- **Hypothesis:** Reverse scan advances rollout_suffix and compaction flag on checkpoint before segment rollback decision; discarded rolled-back checkpoint can still cut off older effective history and suppress prefix baseline restoration.
- **Claimed impact:** Potential loss of complete pre-checkpoint active history, forcing model to rediscover state or repeat tool work; can duplicate context reinjection.
- **Aggregation note:** Distinct from BUGS.md #11 fork baseline and max compound-write windows. Strong but interleaving-specific.

### H082. Compact then resume loses additional-context dedup baseline

- **Lane:** `world-state`
- **Agent confidence:** confirmed
- **Waste class:** resume-triggered,cumulative,multiplicative
- **Evidence:** core/src/compact.rs:61-112,145-165,350-389; core/src/compact_remote.rs:90-108,269-296; core/src/compact_remote_v2.rs:120-137,322-346; core/src/session/additional_context.rs:9-37; core/src/session/mod.rs:1498-1513,3497-3546; core/src/session/rollout_reconstruction.rs:472-486
- **Hypothesis:** DoNotInject compaction rehydrates current additional-context messages into replacement history but persists no post-compact WorldState; restart/fork restores empty store and republishes identical snapshot.
- **Claimed impact:** One redundant up-to-1000-token fragment per current key after boundary; key count uncapped.
- **Aggregation note:** Distinct from BUGS.md #28 raw-before-render fingerprint duplication. Top novel.

### H083. Additional-context keys are unbounded and can evade contextual recognition

- **Lane:** `world-state`
- **Agent confidence:** confirmed
- **Waste class:** per-request,cumulative,multiplicative,compaction-triggered
- **Evidence:** app-server-protocol/src/protocol/v2/turn.rs:112-116,177,292; context-fragments/src/additional_context.rs:6-56,93-101; core/src/state/additional_context.rs:53-84; core/src/context/contextual_user_message.rs:17-39; core/src/event_mapping.rs:58-108; core/src/compact.rs:59,527-570,654-716
- **Hypothesis:** Only values are capped; keys render twice with no count/aggregate bound. A key containing > breaks matches_text, so compaction treats fragment as real user message and rehydration adds another copy.
- **Claimed impact:** Unbounded key bytes; malformed copies can fill 20K retained-user budget plus fresh rehydrated projection.
- **Aggregation note:** Distinct from BUGS.md #28 value fingerprint/environment snapshots. Strong.

### H084. Client-developer retention preserves obsolete application context

- **Lane:** `world-state`
- **Agent confidence:** conditional
- **Waste class:** compaction-triggered,cumulative
- **Evidence:** context-fragments/src/additional_context.rs:59-101; core/src/session/turn_input.rs:640-659; core/src/session/inject.rs:65-112; core/src/compact_remote_v2.rs:77,480-515; core/src/session/mod.rs:3915-3955; core/src/session/additional_context.rs:9-37
- **Hypothesis:** With retention feature, application context becomes generic client developer history; v2/new-window retain every historical version and rehydration does not remove superseded or cleared entries.
- **Claimed impact:** Up to 64K retained-message budget can be obsolete application context.
- **Aggregation note:** Distinct from BUGS.md #28 active update duplication. Feature default-disabled.

### H085. Null extension WorldState snapshots re-emit every inference

- **Lane:** `world-state`
- **Agent confidence:** confirmed
- **Waste class:** per-request,cumulative,multiplicative
- **Evidence:** ext/extension-api/src/contributors/world_state.rs:37-104; core/src/context/world_state/mod.rs:145-180,400-435; core/src/session/mod.rs:3232-3264,3988-4060; core/src/session/turn.rs:223-228,350-380; core/src/context_manager/history.rs:186-200,525-540
- **Hypothesis:** Null snapshots are omitted so section remains Absent; renderer for Absent emits during initial context, before first request and before every inference. Extension body unbounded.
- **Claimed impact:** For B-token fragment over M iterations, active MxB and triangular cumulative input.
- **Aggregation note:** Distinct from BUGS.md #28 built-in snapshot diffs. Strong conditional extension behavior.

### H086. Silent disable transitions duplicate guidance on re-enable

- **Lane:** `world-state`
- **Agent confidence:** confirmed
- **Waste class:** cumulative,multiplicative
- **Evidence:** core/src/context/world_state/apps_instructions.rs:17-51; core/src/context/world_state/plugins_instructions.rs:17-51; core/src/context/world_state/environments_instructions.rs:17-51; core/src/context/world_state/compact_permissions.rs:35-56; core/src/session/mod.rs:3232-3264
- **Hypothesis:** Disable/removal advances snapshot without model-visible tombstone; old fragment remains and re-enable/re-add emits identical fragment again before compaction.
- **Claimed impact:** Hundreds of duplicate tokens per capability cycle; compact permission notice can approach 5KB per cycle.
- **Aggregation note:** Distinct from BUGS.md #28 environment full-field snapshot. Lower impact but direct.

### H087. Remote compaction v1 bypasses shared rollout token budget

- **Lane:** `remote-compaction-v1`
- **Agent confidence:** confirmed
- **Waste class:** budget-bypass,cumulative,multiplicative
- **Evidence:** core/src/tasks/compact.rs:41-58; core/src/session/turn.rs:1218-1255; codex-api/src/endpoint/compact.rs:39-88; core/src/client.rs:569-578,666-677; core/src/compact_remote_request.rs:79-103; core/src/session/mod.rs:4076-4105; core/src/compact_remote_v2.rs:302-307; core/src/rollout_budget.rs:45-64; core/src/session/rollout_budget.rs:25-35; core/tests/suite/rollout_budget.rs:334-404
- **Hypothesis:** V1 compact response exposes no TokenUsage and never records rollout budget, unlike ordinary inference and v2; compaction input/output/budget units do not decrement shared limit and turn can continue.
- **Claimed impact:** Up to entire near-window compaction omitted per attempt; repeated compactions/agents multiply overshoot.
- **Aggregation note:** Distinct from BUGS.md #30 stale reminders and max resume budget reset. Conditional on rollout budget enabled and v2 disabled.

### H088. Copilot WebSocket request normalization is never applied

- **Lane:** `request-construction`
- **Agent confidence:** conditional
- **Waste class:** per-request,cumulative
- **Evidence:** model-provider/src/copilot/auth_provider.rs:139; model-provider/src/copilot/payload.rs:7-33; codex-api/src/endpoint/responses_websocket.rs:273,904-919
- **Hypothesis:** Provider hook intended to trim before latest compaction and normalize Copilot payload has no production caller; raw Responses frame is serialized/sent directly.
- **Claimed impact:** Potentially entire pre-compaction context resent on every later request if raw frame accepted; otherwise failed requests.
- **Aggregation note:** Distinct from BUGS.md #10 retry replay and #20 cache checkpoint. Bypass confirmed; token effect depends Copilot backend acceptance/adapter layer.

### H089. Model switches append byte-identical full instructions

- **Lane:** `request-construction`
- **Agent confidence:** confirmed
- **Waste class:** cumulative,multiplicative
- **Evidence:** core/src/session/mod.rs:671-686; core/src/context/world_state/model.rs:45-58; core/src/client.rs:894-931
- **Hypothesis:** WorldState compares model slugs and appends full destination instructions even when Sol/Terra/Luna instruction bytes are identical to fixed session base; also on first-turn override.
- **Claimed impact:** About 17766 bytes, roughly 4.4K duplicate heuristic tokens per switch plus wrapper.
- **Aggregation note:** Distinct from BUGS.md #28 current-state snapshot duplication. Strong direct duplicate.

### H090. Token recomputation counts provider-stripped passthrough metadata

- **Lane:** `request-construction`
- **Agent confidence:** confirmed
- **Waste class:** compaction-triggered
- **Evidence:** core/src/session/mod.rs:3057-3160,4112-4142; core/src/context_manager/history.rs:725-768; core/src/client.rs:934-943,993-1000; core/src/session/context_window.rs:27-79
- **Hypothesis:** Estimator serializes internal metadata that final non-OpenAI request removes, inflating active context after usage-null responses, compaction or rollback and causing premature compaction.
- **Claimed impact:** About 40 phantom heuristic tokens per representative item; 500 small items about 20K phantom tokens.
- **Aggregation note:** Distinct from max request-shrink finding by specific metadata mismatch. Strong.

### H091. Guardian splits adjacent text into many content-item envelopes

- **Lane:** `request-construction`
- **Agent confidence:** confirmed
- **Waste class:** per-review,cumulative
- **Evidence:** core/src/guardian/prompt.rs:100-103,220-326; protocol/src/models.rs:1948-1966; ext/guardian-v2/src/async_scorer/extension.rs:557-599; ext/guardian-v2/src/async_scorer/sampler.rs:491-510
- **Hypothesis:** Each heading/transcript entry becomes separate input_text item although concatenation preserves text, adding framing and estimator cost; synchronous history retains it.
- **Claimed impact:** 32 serialized bytes per extra part; 40 parts about 312 heuristic tokens, more for tiny-entry transcripts.
- **Aggregation note:** Distinct from max Guardian evidence duplication. Provider tokenization exactness conditional but model-visible framing real.

### H092. Guardian structured-output schema is supplied twice

- **Lane:** `request-construction`
- **Agent confidence:** confirmed
- **Waste class:** per-review,retry-triggered
- **Evidence:** core/src/guardian/prompt.rs:776-823; core/src/guardian/review_session.rs:1180-1184; core/src/client.rs:965-988
- **Hypothesis:** Prose prompt embeds literal JSON schema with same fields/enums/types as final_output_json_schema/text.format.
- **Claimed impact:** Several dozen duplicate input tokens per Guardian inference and retry.
- **Aggregation note:** Distinct from BUGS.md #31 reasoning summaries. Direct duplicate.

### H093. Turn-scoped skill and plugin activations accumulate without aggregate bound

- **Lane:** `skills-plugins-apps`
- **Agent confidence:** confirmed
- **Waste class:** per-request,cumulative,resume-persistent,multiplicative
- **Evidence:** ext/skills/src/host_prompt.rs:76-96; ext/skills/src/extension.rs:457-500; core/src/plugins/render.rs:8-87; core/src/session/turn.rs:280-282,371-390; core/src/session/mod.rs:3175-3220; core/src/context_manager/history.rs:525-538
- **Hypothesis:** Host/repo/system/legacy-plugin skills can bypass 8KB cap; others are per-item capped only. Every activation persists and re-mention appends complete copy despite turn-scoped semantics.
- **Claimed impact:** Bundled prompts 11-19.5KB; imagegen alone about 4.9K tokens per sampling request; filesystem file cap 512MiB and no aggregate.
- **Aggregation note:** Distinct from BUGS.md #13 skills.read pagination. Top novel.

### H094. Skill catalog changes append wholesale stale replacements

- **Lane:** `skills-plugins-apps`
- **Agent confidence:** confirmed
- **Waste class:** change-triggered,cumulative,multiplicative
- **Evidence:** ext/skills/src/world_state.rs:77-118; ext/skills/src/render.rs:17-20,127-152; core/src/context/world_state/apps_instructions.rs:38-49; core/src/context/world_state/plugins_instructions.rs:38-49; core/src/session/mod.rs:3232-3262
- **Hypothesis:** Any catalog mutation emits full new catalog while old remains; false availability emits no removal and re-enable duplicates guidance.
- **Claimed impact:** One mutation up to 5440 metadata tokens at 272K or configured 10K plus framing; app/plugin cycles about 400 tokens.
- **Aggregation note:** Distinct from BUGS.md #28 additional-context/environment path, but overlaps max world silent disable. Merge world-state catalog lifecycle.

### H095. Unavailable plugin discovery exposes whole catalogs

- **Lane:** `skills-plugins-apps`
- **Agent confidence:** confirmed
- **Waste class:** per-request,cumulative,extra-call
- **Evidence:** core/src/session/mod.rs:3666-3689; core/src/context/recommended_plugins_instructions.rs:7-53; core-plugins/src/remote.rs:1036-1081; core/src/tools/handlers/list_available_plugins_to_install_spec.rs:7-19; core/src/tools/handlers/list_available_plugins_to_install.rs:32-102; connectors/src/lib.rs:259-289
- **Hypothesis:** Endpoint mode pins up to 50 unavailable plugin entries in every request; legacy list tool has no query and serializes every candidate including server/app arrays; connector pagination lacks aggregate ceiling.
- **Claimed impact:** Endpoint about 2K-4K tokens/request; legacy can consume full tool-output allowance and hide requested late entry, prompting more discovery.
- **Aggregation note:** Distinct from BUGS.md #1 active tool catalogs. Strong.

### H096. Skill metadata budget excludes duplicated authority framing

- **Lane:** `skills-plugins-apps`
- **Agent confidence:** confirmed
- **Waste class:** per-request,multiplicative
- **Evidence:** ext/skills/src/render.rs:325-345,540-658,716-742,871-895,1087-1107; ext/skills/src/fragments.rs:18-57; ext/skills/src/catalog_prompt.rs:3-40,81-104
- **Hypothesis:** Budget applies only to entry lines; executor/orchestrator/host fragments each add headings, guidance and sometimes full usage instructions outside budget.
- **Claimed impact:** Three authorities add about 2.5K fixed tokens on usage-enabled models, 1.5K-1.8K shareable; Sol still duplicates hundreds; actual fragment can exceed configured 10K.
- **Aggregation note:** Distinct from BUGS.md #1 tool catalog. Moderate.

### H097. Memory phase2 corpus has no aggregate byte or token budget

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** background,cumulative,multiplicative,compaction-triggered
- **Evidence:** memories/write/src/phase1.rs:135-146; memories/write/templates/memories/stage_one_system.md:241-249,401-557; memories/write/src/storage.rs:49-77,115-135; config/src/types.rs:47-56,301-303; memories/write/templates/memories/consolidation.md:119-139,768-780,824-837; memories/write/src/lib.rs:110-113; memories/write/src/workspace.rs:137-170; external-agent-migration/src/memory_import.rs:257-309
- **Hypothesis:** Phase1 outputs unrestricted duplicate raw_memory/rollout_summary strings, selection is count-only, consolidation scans full corpus, diff permits 4MiB and imports are unbounded.
- **Claimed impact:** 4MiB diff alone about 1.05M heuristic tokens of material; large corpus forces repeated reads/model turns/compactions.
- **Aggregation note:** Distinct from BUGS.md #14 retry counter and #2 generic tool outputs. Top novel.

### H098. Every app-server turn starts another memory startup pass

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** background,multiplicative
- **Evidence:** memories/README.md:29-38; app-server/src/request_processors/turn_processor.rs:673-684; memories/write/src/start.rs:54-80; config/src/types.rs:47-56; memories/write/src/phase1.rs:149-175; state/src/runtime/memories.rs:2559-2646
- **Hypothesis:** Pipeline documented as root-session startup is invoked after every newly started input turn; successive passes drain new full batches while backlog exists.
- **Claimed impact:** Two Luna calls per ordinary turn by default, up to 128 configured, while backlog exists.
- **Aggregation note:** First pass omitted as possibly intentional; documentation and successive-batch tests establish lifecycle mismatch. Strong when MemoryTool enabled.

### H099. Memory workers pay oversized fixed prompt taxes

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** background,per-call,multiplicative
- **Evidence:** utils/string/src/truncate.rs:1-4,71-78; memories/write/src/lib.rs:78-93; memories/write/src/phase1.rs:290-319; memories/write/src/prompts.rs:42-80; memories/write/src/phase2.rs:372-378; ext/memories/src/prompts.rs:26-56; ext/memories/src/extension.rs:51-77
- **Hypothesis:** Stage1 template about 7613 tokens per rollout; phase2 template about 12810 tokens in one item; read template about 1649 plus summary. Repeated explanatory sections are sent each call/follow-up.
- **Claimed impact:** Max phase1 pass about 974K fixed prompt tokens before rollout content/cache; phase2 exceeds 10K single-item rule and persists across tool follow-ups.
- **Aggregation note:** Distinct from BUGS.md #5 metadata workers and #13 skill reads. Top novel.

### H100. Memory consolidation has no wall-clock step or token ceiling

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** background,multiplicative
- **Evidence:** memories/write/src/phase2.rs:395-420,498-552; core/src/session/turn.rs:300-476; core/src/config/mod.rs:1035-1038; memories/write/src/start.rs:54-81
- **Hypothesis:** Detached worker heartbeats one-hour lease every 90s indefinitely while ordinary agent loop continues valid tools/compactions; rollout budget optional and no cancellation handle.
- **Claimed impact:** Unbounded Terra requests and compaction calls under continuing tool behavior.
- **Aggregation note:** First pass candidate omitted from BUGS; distinct from BUGS #14 retry counter. Strong conditional on model loop.

### H101. Phase1 leases can expire before queued jobs start, duplicating live calls

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** background,retry-triggered,multiplicative
- **Evidence:** memories/write/src/lib.rs:78-84; memories/write/src/phase1.rs:149-175,205-220,228-277,370-402; state/src/runtime/memories.rs:671-830,1807-1858
- **Hypothesis:** Whole batch is claimed with one-hour leases then only eight run concurrently; no heartbeat/ownership check during sampling, so later pass can steal expired queued/running job while original future continues.
- **Claimed impact:** Duplicate extraction calls after lease expiry; repeated hourly takeovers possible.
- **Aggregation note:** Distinct from BUGS.md #14 phase2 retry and known Stop fix. Strong.

### H102. Memory quota guard fails open

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** background,multiplicative
- **Evidence:** config/src/types.rs:308-315; memories/write/src/guard.rs:9-39; memories/write/src/start.rs:63-80
- **Hypothesis:** Auth absence, quota lookup failure, empty/unusable snapshot all become allow=true; check runs once before phase1 batch and phase2.
- **Claimed impact:** Failure can release up to 128 Luna calls plus Terra consolidation; no recheck after phase1 consumes quota.
- **Aggregation note:** Distinct from BUGS.md transport retries. Strong.

### H103. Parsed memory citations remain in model history and extraction

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** cumulative,background
- **Evidence:** ext/memories/templates/memories/read_path.md:75-115; core/src/stream_events_utils.rs:44-57,91-98,451-465; core/src/session/turn.rs:371-390; memories/write/src/phase1.rs:404-448
- **Hypothesis:** UI turn item strips/parses citation block, but original assistant ResponseItem is recorded and resent; phase1 later retains it despite structured metadata already existing.
- **Claimed impact:** Citation entries and rollout IDs on every later request and another memory extraction.
- **Aggregation note:** Distinct from BUGS.md #28 context fragments. Lower impact.

### H104. Memory consolidation final assistant answer is generated then discarded

- **Lane:** `memory-workers`
- **Agent confidence:** confirmed
- **Waste class:** background,output
- **Evidence:** memories/write/templates/memories/consolidation.md:179-186,879-880; core/src/agent/status.rs:6-12; memories/write/src/phase2.rs:399-414
- **Hypothesis:** Worker prompt lacks minimal sentinel contract; completed final message is stored in status but phase2 checks variant/usage and discards content.
- **Claimed impact:** Any final recap output tokens have no consumer.
- **Aggregation note:** Distinct from BUGS.md #31 Guardian summary. Likely modest but direct.

### H105. Memory extraction converts retained media into base64 text

- **Lane:** `images-multimodal`
- **Agent confidence:** confirmed
- **Waste class:** background,per-rollout,cumulative
- **Evidence:** memories/write/src/phase1.rs:282-320,404-468; rollout/src/policy.rs:65-90; memories/write/src/prompts.rs:100-125
- **Hypothesis:** Phase1 clones media-bearing messages/tool outputs from raw rollout, JSON-serializes them into InputText, and ignores compacted checkpoints, so typed media removed from active history becomes tokenized base64 text.
- **Claimed impact:** One 1MiB image about 350K heuristic text tokens before truncation, potentially whole 70 percent stage1 allowance; 2 rollouts default, 128 configured.
- **Aggregation note:** Distinct from BUGS.md #16 audio retention and max memory discovery schemas. Top novel, MemoryTool-gated.

### H106. Durationless valid audio is estimated as base64 text

- **Lane:** `images-multimodal`
- **Agent confidence:** confirmed
- **Waste class:** per-item,compaction-triggered,extra-call
- **Evidence:** utils/audio/src/lib.rs:149-186,268-350; core/src/context_manager/history.rs:674-684; utils/output-truncation/src/lib.rs:164-183
- **Hypothesis:** Accepted containers without duration metadata fall back to approx_token_count(audio_url), charging base64 text rather than duration and omitting valid audio from result or inflating context.
- **Claimed impact:** 10s 64kbps WebM example charged about 41128 tokens versus intended 100, over 400x and beyond 12K output allowance.
- **Aggregation note:** Distinct from BUGS.md #16 zero-cost retained audio. Strong.

### H107. Unsupported media is charged before request normalization strips it

- **Lane:** `images-multimodal`
- **Agent confidence:** confirmed
- **Waste class:** compaction-triggered,extra-call
- **Evidence:** core/src/context_manager/history.rs:208-225,269-282,400-459; core/src/context_manager/normalize.rs:328-409; core/src/session/turn.rs:425-499
- **Hypothesis:** History accounting charges full media under current stored representation; only later prompt clone replaces unsupported media with tiny markers, causing premature compaction.
- **Claimed impact:** One output can add about 12K phantom tokens; four high-detail images about 7.4K versus short markers.
- **Aggregation note:** Duplicate of max context request-shrink/history normalization candidate; distinct from BUGS.md #15 incoming media. Merge.

### H108. Guardian V2 can include same REPL screenshot twice

- **Lane:** `images-multimodal`
- **Agent confidence:** conditional
- **Waste class:** background,per-classification,retry-multiplicative
- **Evidence:** core/src/tools/handlers/mcp.rs:300-390; ext/guardian-v2/src/async_scorer/transcript.rs:83-156; ext/guardian-v2/src/extension.rs:483-532; ext/guardian-v2/src/async_scorer/sampler.rs:489-647
- **Hypothesis:** Screenshot can exist in conversation history and NodeReplReviewEvidence; separate collection passes append both with no cross-source dedup and retries resend.
- **Claimed impact:** About 1.8K duplicate tokens for common high-detail screenshot, up to 10K original; three attempts can triple.
- **Aggregation note:** Distinct from max Guardian action duplication and BUGS.md #31 summary. Strong if screenshot emitted to both sources.

### H109. Guardian policy and classifier instructions are aggregate-unbounded

- **Lane:** `reasoning-reviewers`
- **Agent confidence:** conditional
- **Waste class:** background,per-review,retry-multiplicative
- **Evidence:** core/src/config/mod.rs:1504-1516; core/src/guardian/review_session.rs:1398-1406; ext/guardian-v2/src/async_scorer/config.rs:42-43,301-304
- **Hypothesis:** Managed/catalog policy strings flow into synchronous Guardian without cap; Guardian V2 defaults classifier instruction limit to none.
- **Claimed impact:** Bundled sync template plus policy about 4.5K tokens; configured values code-unbounded and resent on new threads/retries/policy changes.
- **Aggregation note:** Distinct from BUGS.md #31 unused reasoning summaries. Agent-reported hypothesis, not adjudicated.

### H110. Materialized goal attachments force fetch rounds and encourage rereads

- **Lane:** `goals`
- **Agent confidence:** likely
- **Waste class:** extra-call,cumulative,tool-output
- **Evidence:** tui/src/goal_files.rs:20-185; tui/src/app/thread_goal_actions.rs:120-215; ext/goal/src/runtime.rs:385-415
- **Hypothesis:** Pastes/images and objectives over 4000 chars become file paths/references; initial model must fetch content, and every continuation repeats reference while prior read output remains.
- **Claimed impact:** At least one model-tool-model fetch round; repeated rereads and attachment size have no goal-specific aggregate limit.
- **Aggregation note:** Distinct from BUGS.md #4 polling and max goal continuation text. Agent-reported hypothesis, not adjudicated.

### H111. Remote V2 rereads consumed tool output and only emergency-trims a trailing output run

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** confirmed
- **Waste class:** per-compaction,cumulative,retry-multiplicative
- **Evidence:** core/src/compact_remote_v2_attempt.rs:40-85; core/src/compact_remote.rs:399-436; core/src/compact_remote_v2.rs:480-508
- **Hypothesis:** Ordinary V2 compaction sends consumed file reads, shell output, searches, MCP results, and tool-search output because trimming starts only above the full context limit. Once trimming starts, the first non-rewritable history group stops traversal, so older outputs remain even though the installed checkpoint discards them.
- **Claimed impact:** A 150K-200K coding context can spend most of its compaction input on retired tool evidence; several 10K-12K outputs can contribute tens or hundreds of thousands of tokens per attempt.
- **Aggregation note:** Imported unverified from `imported BUGS.updated.md`, audited there against snapshot `71f3e8fba2414f2418a7bdfe442bfe9acc986124`. Broader than BUGS.md #18 and H039.

### H112. Persisted hook prompts can survive Remote V2 compaction

- **Lane:** `hooks`
- **Agent confidence:** confirmed
- **Waste class:** checkpoint-persistent,cumulative,multiplicative
- **Evidence:** core/src/compact_remote.rs:354-379; core/src/compact_remote_v2.rs:480-508; core/src/hook_runtime.rs:764-783
- **Hypothesis:** Persisted user-role `HookPrompt` messages are retained verbatim by Remote V2, allowing per-fragment or unlimited hook context to consume the next window's 64K retained-message allowance instead of expiring at the checkpoint.
- **Claimed impact:** Up to the complete retained-message allowance can remain occupied by historical hook prompts, extending their cost across all post-compaction requests.
- **Aggregation note:** Imported unverified from `imported BUGS.updated.md`, snapshot `71f3e8fba2414f2418a7bdfe442bfe9acc986124`. Extends BUGS.md #8 beyond the active window.

### H113. A lost terminal event can regenerate an already completed Remote V2 compaction

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** confirmed
- **Waste class:** retry-triggered,input-and-output-multiplicative
- **Evidence:** core/src/compact_remote_v2.rs:375-457; core/src/responses_retry.rs:81-111; core/tests/suite/compact_remote.rs:1832-1935
- **Hypothesis:** The collector can receive and store a complete `ResponseItem::Compaction`, then return a retryable error solely because `response.completed` never arrives. The generated item is discarded and the entire semantic compaction operation is submitted again under the uncapped WebSocket retry path.
- **Claimed impact:** Multiple complete compaction inputs and generated compaction outputs can be paid even though the first compacted artifact already reached the client.
- **Aggregation note:** Imported unverified from `imported BUGS.updated.md`, snapshot `71f3e8fba2414f2418a7bdfe442bfe9acc986124`. More specific than BUGS.md #10.

### H114. Inline Remote V2 can lose `previous_response_id` reuse and resend the full context

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** confirmed
- **Waste class:** per-compaction,full-context-replay,cache-loss
- **Evidence:** core/src/client.rs:330-385,1311-1352,965-986; core/src/compact_remote_v2_attempt.rs:40-85
- **Hypothesis:** V2 hardcodes no output schema and may rewrite historical tool outputs. A preceding structured-output request changes the cache-relevant `text` property, while emergency rewriting changes prior input items. Either difference prevents incremental continuation and turns a trigger-sized compaction delta into a complete-context request.
- **Claimed impact:** The difference can be hundreds of thousands of input tokens for one near-window checkpoint.
- **Aggregation note:** Imported unverified from `imported BUGS.updated.md`, snapshot `71f3e8fba2414f2418a7bdfe442bfe9acc986124`. Distinct from H030 service-tier transitions and H071 hook splicing.

### H115. Remote V2 can retain large coordination messages across the checkpoint

- **Lane:** `agent-status-paths`
- **Agent confidence:** confirmed
- **Waste class:** checkpoint-persistent,cumulative,multiplicative
- **Evidence:** core/src/compact_remote_v2.rs:75-78,520-557
- **Hypothesis:** Non-descendant-progress, non-final `AgentMessage` coordination items can survive V2 compaction with an individual 10K-token ceiling, even when their lifecycle purpose ended before the checkpoint.
- **Claimed impact:** In a large agent workload, several 10K coordination messages can consume most of the 64K retained-message allowance before the opaque compaction item and fresh context are added.
- **Aggregation note:** Imported unverified from `imported BUGS.updated.md`, snapshot `71f3e8fba2414f2418a7bdfe442bfe9acc986124`. Extends BUGS.md #23 beyond list/wait output replay.

### H116. Remote V2 model fallback can repeat the complete compaction on a second model

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** conditional
- **Waste class:** retry-triggered,full-operation-multiplicative
- **Evidence:** core/src/compact_remote_v2.rs:245-292; core/src/compact_model_fallback.rs:8-19
- **Hypothesis:** Invalid request, unexpected status, context-window, usage-limit, overload, internal-server, and retry-limit errors can rerun `run_remote_compact_v2_attempt` with the current model rather than resume the first operation. If the first model accepted or partly processed the request, the second model repeats its full input and output work.
- **Claimed impact:** Up to one additional complete V2 compaction request and generated output per fallback event, before transport retries within either attempt.
- **Aggregation note:** Imported unverified from `imported BUGS.updated.md`, snapshot `71f3e8fba2414f2418a7bdfe442bfe9acc986124`.

### H117. Remote V2 pays for non-compaction assistant output that it discards

- **Lane:** `remote-compaction-v2`
- **Agent confidence:** confirmed
- **Waste class:** per-compaction,output,retry-multiplicative
- **Evidence:** core/src/compact_remote_v2.rs:417-477; core/tests/suite/compact_remote.rs:1938-2013
- **Hypothesis:** V2 consumes every `OutputItemDone` but installs only the single `ResponseItem::Compaction`. Assistant output generated alongside the compaction item is paid output and then dropped.
- **Claimed impact:** Every auxiliary assistant response adds pure output-token cost; retries or model fallback repeat it.
- **Aggregation note:** Imported unverified from `imported BUGS.updated.md`, snapshot `71f3e8fba2414f2418a7bdfe442bfe9acc986124`. Complements BUGS.md #17 and H014.
