# Token-Efficiency Hardening

Defensive work identified by the token-consumption audit. These items protect against
malformed, adversarial, extreme, crash, race, or recovery behavior, but the current
evidence does not establish recurring or direct model-token burn.

The demonstrated token-burning defects are tracked in [BUGS.md](BUGS.md). Product and
provider choices are tracked in [DESIGN_DECISIONS.md](DESIGN_DECISIONS.md). Detailed
canonical merge analysis, source mappings, and preserved fleet adjudication
evidence are in [TOKEN_AUDIT_EVIDENCE.md](TOKEN_AUDIT_EVIDENCE.md).

## Active hardening and resilience facets

**Summary:** 22 hardening facets. Facet labels are authoritative where one common-fix
ID also contains a demonstrated bug or a design decision.

| Canonical ID | Title & Summary | Hardening Seam |
| --- | --- | --- |
| CF-004 | All hook-generated model context consumes one aggregate budget | `codex-rs/hooks/src/output_spill.rs` & `core/src/hook_runtime.rs` |
| CF-013 | External tool schemas preserve executable constraints | `codex-rs/tools/src/json_schema.rs` & `tools/src/responses_api.rs` |
| CF-014 (cursor/paging) | MCP resource listing paging and cursor guards | `codex-rs/core/src/tools/handlers/mcp_resource.rs` |
| CF-015 (oversized-read continuation) | Progressive recovery for oversized MCP resource reads | `codex-rs/core/src/tools/handlers/mcp_resource/read_mcp_resource.rs` |
| CF-017 (under-admission) | Admission accounting for late-added request fields | `codex-rs/core/src/context_manager/history.rs` |
| CF-021 (retained-envelope) | Full envelope and metadata accounting in Remote V2 | `codex-rs/core/src/compact_remote_v2.rs::truncate_retained_messages` |
| CF-035 | Bound standalone web-search history input | `codex-rs/ext/web-search/src/history.rs::recent_input` |
| CF-041 | Bound the DB-backed Phase 2 corpus | `codex-rs/state/src/runtime/memories.rs::get_phase2_input_selection` |
| CF-043 | Enforce a hard Phase 2 execution deadline | `codex-rs/memories/write/src/phase2.rs::agent::loop_agent` |
| CF-047 | Bound external-agent memory imports | `codex-rs/external-agent-migration/src/memory_import.rs` |
| CF-050 | Bound Stop continuation rounds | `codex-rs/core/src/session/turn.rs::run_turn` |
| CF-055 | Reject or encode top-level-null World State snapshots | `codex-rs/ext/extension-api/src/contributors/world_state.rs` |
| CF-058 | Aggregate explicit skill and plugin activation admission | `codex-rs/core/src/session/turn.rs::build_skills_and_plugins` |
| CF-063 (final-budget-accounting) | Charge complete rendered skill catalogs to budget | `codex-rs/ext/skills/src/render.rs::render_combined_available_skills` |
| CF-068 | Finalize Unified Exec output only after producer closure | `codex-rs/core/src/unified_exec/process_manager.rs` |
| CF-072 | Bound Code Mode notification size, count, and backlog | `codex-rs/code-mode-runtime/src/runtime/callbacks.rs` |
| CF-079 | Harden nested-parent completion delivery against residency eviction | `codex-rs/core/src/agent/control/residency.rs` |
| CF-087 | Hard-cap fully rendered Guardian instructions | `codex-rs/ext/guardian-v2/src/async_scorer/config.rs` |
| CF-095 | Report bounded deferred-search partiality | `codex-rs/tools/src/tool_discovery.rs` |
| CF-099 | Persist compound compaction and context state atomically | `codex-rs/core/src/session/mod.rs` & `thread-store` |
| CF-118 | Bound aggregate effective realtime instructions | `codex-rs/core/src/realtime_conversation.rs` |
| CF-121 | Close core-owned Frameless sessions explicitly | `codex-rs/core/src/realtime_conversation.rs` |

---

## Detailed Hardening Records

### CF-004: All Hook-Generated Model Context Consumes One Aggregate Budget

- **Sources & Facet:** #8, H007
- **Failure Mode & Risk:** Ordinary additional context and fragments flattened into Stop prompts are independently granted full per-handler allowances (e.g. 2,500 tokens). Setting the limit to 0 can disable spilling entirely (`codex-rs/hooks/src/output_spill.rs:12-23`, `codex-rs/core/src/hook_runtime.rs:764-775`, `codex-rs/hooks/src/events/stop.rs:410-445`). If many trusted handlers run in parallel, their combined output can overflow model context.
- **Why Hardening (Not Direct Burn):** Admitted hook fragments are intentional extensions; the issue is missing aggregate defense-in-depth against large multi-handler batches rather than demonstrated baseline token waste.
- **Hardening Seam & Implementation:** Add a hard `HookModelContextBudget` spanning sync/async additional context and Stop prompt admission in `codex-rs/core/src/hook_runtime.rs` and `turn.rs::run_turn`.
- **Required Verification:** Test aggregate sync, async, mixed, zero-config, and Stop-batch cases; assert combined payload cannot exceed turn budget.

### CF-013: External Tool Schemas Preserve Executable Constraints

- **Sources & Facet:** H050, H068
- **Failure Mode & Risk:** `parse_tool_input_schema` and `sanitize_json_schema` (`codex-rs/tools/src/json_schema.rs:39-77, 477-556`) strip numeric bounds, string lengths, array cardinality, and boolean-schema shapes during JSON Schema lowering. If the model generates parameters violating the original schema, the tool execution fails on the external server, requiring a retry round.
- **Why Hardening (Not Direct Burn):** Corrective inference requires a model violation plus external server rejection; the direct defect is schema fidelity and validation compatibility.
- **Hardening Seam & Implementation:** Preserve JSON-Schema constraints in a canonical wrapper and perform provider capability lowering only at `responses_api.rs::tool_definition_to_responses_api_tool`.
- **Required Verification:** Round-trip numeric, string, array, and boolean-schema forms for MCP and dynamic tools; verify server/client enforcement.

### CF-014: MCP Resource Listing Paging and Cursor Guards

- **Sources & Facet:** H059 (resource listing facet), H060 (cursor/paging facet)
- **Failure Mode & Risk:** Single-server resource listing lacks bounds on cursor size/repetition (`codex-rs/core/src/tools/handlers/mcp_resource.rs:104-186`), while all-server listing truncates in the middle without a continuation handle.
- **Why Hardening (Not Direct Burn):** Defensive bounding of malformed or infinite cursor loops and pagination resilience.
- **Hardening Seam & Implementation:** Unified resource pager with cursor size/repetition validation and composite continuation tokens.
- **Required Verification:** Test oversized resource lists and cyclic cursors; assert clean pagination and rejection of invalid cursors.

### CF-015: Progressive Recovery for Oversized MCP Resource Reads

- **Sources & Facet:** H059 (oversized read facet) (oversized-read continuation facet)
- **Failure Mode & Risk:** Oversized resource reads are truncated destructively without range or continuation headers, so a model attempting to read the omitted content has no structured way to request the next slice.
- **Why Hardening (Not Direct Burn):** Token waste requires the model to re-attempt the read; progressive chunking provides structured recovery.
- **Hardening Seam & Implementation:** Progressive range-based resource streaming in `ReadMcpResourceHandler`.
- **Required Verification:** Read a resource exceeding the output cap; verify continuation token is returned and subsequent chunk can be read.

### CF-017: Admission Accounting for Late-Added Request Fields

- **Sources & Facet:** #3, #15 (under-admission facet)
- **Failure Mode & Risk:** `estimate_token_count_with_base_instructions` omits tool schemas and structured output schemas added late in request construction. If history plus schemas exceed the window, the request is rejected by the provider.
- **Why Hardening (Not Direct Burn):** The rejected request does not generate output; extra tokens depend on caller retry behavior.
- **Hardening Seam & Implementation:** Accurate admission accounting including all final request fields before dispatch.
- **Required Verification:** Construct a request where history fits but tools push it over limit; verify admission catches the overflow before network call.

### CF-021: Full Envelope and Metadata Accounting in Remote V2

- **Sources & Facet:** H016 (retained-envelope facet)
- **Failure Mode & Risk:** Remote V2 retention estimates message text but omits roles, item IDs, and wrapper metadata (`codex-rs/core/src/compact_remote_v2.rs:601-733`). In extreme histories with thousands of tiny messages, metadata overhead can push post-compaction history over budget.
- **Why Hardening (Not Direct Burn):** Framing is required content; bounding protects against extreme message counts.
- **Hardening Seam & Implementation:** Budget retained messages using the full serialized item estimator in `truncate_retained_messages`.
- **Required Verification:** Test retention with 10,000 tiny messages; verify post-compaction request fits within target context window.

### CF-035: Bound Standalone Web-Search History Input

- **Sources & Facet:** H056
- **Failure Mode & Risk:** Standalone web search forwards the two latest real user messages without an aggregate byte limit (`codex-rs/ext/web-search/src/history.rs:12-25`), only capping assistant text.
- **Why Hardening (Not Direct Burn):** User text is legitimate query context; an aggregate cap is defensive bounding against massive pastes.
- **Hardening Seam & Implementation:** Enforce an aggregate `SearchHistoryBudget` in `recent_input`.
- **Required Verification:** Test standalone search with two 50K-character user messages; verify input is truncated to the aggregate cap.

### CF-041: Bound the DB-Backed Phase 2 Corpus

- **Sources & Facet:** H097 (DB corpus facet)
- **Failure Mode & Risk:** `get_phase2_input_selection` (`codex-rs/state/src/runtime/memories.rs:446-523`) bounds corpus row count and age, but lacks an aggregate byte/token limit.
- **Why Hardening (Not Direct Burn):** Protects against runaway consolidation prompts from unusually large model-generated memory entries.
- **Hardening Seam & Implementation:** Apply an aggregate byte/token ceiling to Phase 2 corpus selection.
- **Required Verification:** Populate DB with 1,000 large memory rows; verify selected corpus does not exceed the aggregate limit.

### CF-043: Enforce a Hard Phase 2 Execution Deadline

- **Sources & Facet:** H100
- **Failure Mode & Risk:** The Phase 2 consolidation worker polls and heartbeats until reaching a terminal state with no global wall-clock deadline (`codex-rs/memories/write/src/phase2.rs:492-554`).
- **Why Hardening (Not Direct Burn):** Failsafe against runaway agent loops.
- **Hardening Seam & Implementation:** Enforce a total execution timeout and maximum step ceiling on `loop_agent`.
- **Required Verification:** Simulate a non-terminating Phase 2 loop; verify execution aborts cleanly at the deadline.

### CF-047: Bound External-Agent Memory Imports

- **Sources & Facet:** H097 (import facet)
- **Failure Mode & Risk:** `memory_import.rs::import` recursively imports memory files from external agent directories without aggregate file count or byte limits (`281-318`).
- **Why Hardening (Not Direct Burn):** Untrusted input bounding for migration tooling.
- **Hardening Seam & Implementation:** Enforce count and size limits on imported memory files.
- **Required Verification:** Import a directory with 10,000 files; verify import is capped with clear warnings.

### CF-050: Bound Stop Continuation Rounds

- **Sources & Facet:** #9
- **Failure Mode & Risk:** Stop hooks can return blocking reasons requesting continuation (`codex-rs/core/src/session/turn.rs:502-537`). Malfunctioning hooks can create infinite sampling loops.
- **Why Hardening (Not Direct Burn):** Host-side ceiling against misbehaving extensions.
- **Hardening Seam & Implementation:** Enforce a maximum Stop continuation round limit (e.g. 5 rounds) in `run_turn`.
- **Required Verification:** Create a hook that always blocks; verify turn terminates with an error after the configured maximum rounds.

### CF-055: Reject or Encode Top-Level-Null World State Snapshots

- **Sources & Facet:** H085
- **Failure Mode & Risk:** `WorldStateSectionContribution::new` accepts top-level null JSON, which Core normalizes to `Absent`, causing extension sections to re-emit repeatedly (`codex-rs/core/src/context/world_state/mod.rs:145-151`).
- **Why Hardening (Not Direct Burn):** Extension API input validation.
- **Hardening Seam & Implementation:** Reject top-level null contributions or represent them explicitly as empty snapshots.
- **Required Verification:** Provide a null snapshot from an extension; verify it does not re-emit on every turn.

### CF-058: Aggregate Explicit Skill and Plugin Activation Admission

- **Sources & Facet:** H093 (same-turn aggregate facet)
- **Failure Mode & Risk:** Explicit skill and plugin activations are individually capped but concatenated without an aggregate budget (`codex-rs/core/src/session/turn.rs:773-930`).
- **Why Hardening (Not Direct Burn):** Protects against context overflow when many skills/plugins are activated simultaneously.
- **Hardening Seam & Implementation:** Impose an aggregate token budget across all active skill/plugin prompts in `build_skills_and_plugins`.
- **Required Verification:** Activate 20 large skills in one turn; verify aggregate prompt size remains within configured limit.

### CF-063: Charge Complete Rendered Skill Catalogs to Budget

- **Sources & Facet:** H096 (budget facet) (final-budget-accounting facet)
- **Failure Mode & Risk:** Skill catalog line budgeting checks entries before adding authority headers and XML wrapper tags (`codex-rs/ext/skills/src/render.rs:540-655`).
- **Why Hardening (Not Direct Burn):** Ensures final serialized output strictly respects the configured catalog budget.
- **Hardening Seam & Implementation:** Account for rendered framing and wrapper overhead in `render_combined_available_skills`.
- **Required Verification:** Render a catalog near the budget ceiling; verify the total serialized token count does not exceed the limit.

### CF-068: Finalize Unified Exec Output Only After Producer Closure

- **Sources & Facet:** H010
- **Failure Mode & Risk:** `collect_output_until_deadline` waits only 50ms after process exit for output streams to drain (`codex-rs/core/src/unified_exec/process_manager.rs:1402-1456`). Slow pipes can lose trailing output.
- **Why Hardening (Not Direct Burn):** Prevents subtle race conditions in command output capture.
- **Hardening Seam & Implementation:** Await explicit pipe EOF before marking process output finalized.
- **Required Verification:** Run a command that sleeps briefly before writing final stdout on exit; assert all bytes are captured.

### CF-072: Bound Code Mode Notification Size, Count, and Backlog

- **Sources & Facet:** #12 (notification budget facet)
- **Failure Mode & Risk:** `notify_callback` accepts arbitrary text on an unbounded channel (`codex-rs/code-mode-runtime/src/runtime/callbacks.rs:265-289`), which can overflow memory and context if a script runs away.
- **Why Hardening (Not Direct Burn):** Defensive resource bounding against misbehaving user scripts.
- **Hardening Seam & Implementation:** Apply per-notification size caps, rate limits, and bounded channel buffers in `CodeModeDispatchBroker`.
- **Required Verification:** Flood 1,000 notifications from a cell; verify backlog is capped and excess notifications are dropped with warnings.

### CF-079: Harden Nested-Parent Completion Delivery Against Residency Eviction

- **Sources & Facet:** H047
- **Failure Mode & Risk:** LRU residency eviction can unload an idle parent agent while its child is still executing (`codex-rs/core/src/agent/control/residency.rs:123-154`). When the child completes, delivery fails.
- **Why Hardening (Not Direct Burn):** Cross-process agent lifecycle resilience.
- **Hardening Seam & Implementation:** Mark parents with active children as non-evictable or persist completion messages durably in a parent mailbox.
- **Required Verification:** Evict a parent agent while child runs; verify child completion is delivered upon parent resume.

### CF-087: Hard-Cap Fully Rendered Guardian Instructions

- **Sources & Facet:** H109
- **Failure Mode & Risk:** Guardian classifier policy and instructions are trimmed but lack a hard aggregate ceiling (`codex-rs/core/src/config/mod.rs:1504-1516`).
- **Why Hardening (Not Direct Burn):** Protects against extreme or adversarial Guardian configuration files.
- **Hardening Seam & Implementation:** Enforce a hard token ceiling (e.g. 10K tokens) on rendered Guardian instructions in `render_classifier_instructions`.
- **Required Verification:** Configure a 50K-token Guardian policy; verify policy is safely truncated with an error.

### CF-095: Report Bounded Deferred-Search Partiality

- **Sources & Facet:** H040
- **Failure Mode & Risk:** `bound_tool_search_output` (`codex-rs/tools/src/tool_discovery.rs:17-64`) truncates search results at 32 KiB and reports `status: completed` even when matches are omitted.
- **Why Hardening (Not Direct Burn):** Informational completeness and pagination readiness.
- **Hardening Seam & Implementation:** Return `status: partial` with total match count and pagination hints when results are truncated.
- **Required Verification:** Execute a search matching 100 tools; verify response indicates partiality and reports remaining count.

### CF-099: Persist Compound Compaction and Context State Atomically

- **Sources & Facet:** H074 (compound persistence facet), H113 (failed persistence facet)
- **Failure Mode & Risk:** Compacted history, World State, and Turn Context are written as separate non-transactional appends (`codex-rs/core/src/session/mod.rs:3496-3544`). A crash midway creates a corrupt split state.
- **Why Hardening (Not Direct Burn):** Transactional storage integrity and crash resilience.
- **Hardening Seam & Implementation:** Commit compound compaction records and updated baselines in a single atomic database transaction.
- **Required Verification:** Inject a crash after writing compacted history but before writing World State; verify recovery rejects partial record.

### CF-118: Bound Aggregate Effective Realtime Instructions

- **Sources & Facet:** H043
- **Failure Mode & Risk:** Realtime start validates individual instruction fields but lacks an aggregate ceiling on combined system instructions and startup overrides (`codex-rs/core/src/realtime_conversation.rs:1315-1357`).
- **Why Hardening (Not Direct Burn):** Defensive input bounding for realtime sessions.
- **Hardening Seam & Implementation:** Enforce a hard aggregate character/token limit in `prepare_realtime_start`.
- **Required Verification:** Submit 50K of combined realtime instructions; verify configuration is rejected with a validation error.

### CF-121: Close Core-Owned Frameless Sessions Explicitly

- **Sources & Facet:** H046 (core-created Frameless facet)
- **Failure Mode & Risk:** Replacing a realtime conversation cancels local tasks but omits sending `session.close` to the WebRTC peer (`codex-rs/core/src/realtime_conversation.rs:528-542`).
- **Why Hardening (Not Direct Burn):** Clean transport lifecycle cleanup.
- **Hardening Seam & Implementation:** Invoke `session.close` on core-owned Frameless handles during session teardown.
- **Required Verification:** Replace a realtime session; verify `session.close` message is sent across the transport.
