# Token-Efficiency Design Decisions

Product, quality, ordering, security, cache, and provider-contract choices identified by
the token-consumption audit. The token cost can be real or plausible, but removing it
would choose behavior that current code and contracts do not uniquely determine.

These entries are decision records, not authorized bug fixes. The demonstrated
token-burning defects are tracked in [BUGS.md](BUGS.md), and defensive work is tracked
in [HARDENING.md](HARDENING.md). Detailed canonical merge analysis, source mappings,
and preserved fleet adjudication evidence are in
[TOKEN_AUDIT_EVIDENCE.md](TOKEN_AUDIT_EVIDENCE.md).

## Active design and provider-contract facets

**Summary:** 51 design or provider-contract facets. Facet labels are authoritative
where one common-fix ID also contains a demonstrated bug or hardening work.

| Canonical ID | Title & Summary | Required Gate / Decision |
| --- | --- | --- |
| CF-001 (metadata-bootstrap) | Metadata worker context profile and bootstrap inclusion | Product decision & output quality validation |
| CF-011 | Budget-aware model tool catalog planning (direct vs deferred) | Product decision on catalog budget heuristics |
| CF-016 (annotation-policy) | MCP audience and priority annotation filtering policy | Product decision on annotation authority |
| CF-020 (durable-media-policy) | Model-independent durable media representation and reversibility | Product decision on storage vs cross-model fidelity |
| CF-023 | Automation recurrence prompt-cache affinity keying | Product decision on cache isolation boundaries |
| CF-024 | Turn idempotency key semantics and protocol contract | API review and v2 turn protocol approval |
| CF-026 | Automation source eligibility for memory extraction | Product decision on automated cron memory capture |
| CF-029 | Latest-state reminder projection vs append-only audit trail | Product decision on cache hit rate vs context tokens |
| CF-031 | Goal budget authority over descendant agents | Product decision on hierarchical budget governance |
| CF-032 | Active-turn semantics for goal mutation and replacement | Product decision on goal cancellation vs turn completion |
| CF-034 (persistence-failure policy) | Goal accounting persistence failure recovery policy | Product approval on fail-closed vs retry policies |
| CF-036 | Goal attachment transport (materialized references vs typed inline objects) | Product decision on inline threshold vs file references |
| CF-037 | Memory startup scheduling (once per root thread vs per turn) | Product decision on memory backlog processing cadence |
| CF-039 | Condensing Phase 1 extraction rubric vs recall quality | Extraction recall quality benchmarking & approval |
| CF-040 | Memory quota lookup failure policy (fail-open vs fail-closed) | Product decision on quota failure handling policy |
| CF-042 | Mode-specific Phase 2 consolidation prompt splitting | Product approval for prompt template split |
| CF-045 | General memory evidence source after excluding retired schemas | Product decision on raw audit vs compacted semantic evidence |
| CF-049 | Sanitized prompt projections while preserving raw citation provenance | Architecture decision on durable raw vs model-visible projections |
| CF-051 | Internal worker hook profile (exclusions for review and memory workers) | Security & product decision on worker hook boundaries |
| CF-053 | Completed Stop prompt expiry at V2 compaction checkpoints | Product decision on Stop hook audit durability |
| CF-057 | skills.read completeness vs total resource exposure cap | Product decision on maximum allowed skill resource size |
| CF-059 | Turn-scoped skill reactivation (references vs full bodies) | Quality benchmarking on skill reference tags |
| CF-061 | Eager vs lazy endpoint plugin recommendations | Product decision on plugin discovery UX vs initial tokens |
| CF-062 | Query and pagination API for legacy plugin discovery | App-server API schema review and versioning approval |
| CF-063 (authority-guidance) | Coalescing authority-specific guidance in skill catalogs | Security review on skill provenance clarity |
| CF-064 | Unified Exec wait model (host-owned async wait vs bounded polling yields) | Product decision on async execution UX vs timeouts |
| CF-065 | Code Mode wait model (host-owned completion vs bounded timeout yields) | Runtime architecture approval for long-lived wait holds |
| CF-066 | Terminal output normalization policy (PTY echo, control sequences) | Product decision on terminal text fidelity |
| CF-070 | Hybrid Code Mode schema presentation (dual modality vs exec only) | Product decision on Code Mode invocation paradigms |
| CF-074 | Agent status API response format (full message bodies vs acknowledged references) | Subagent API v2 protocol design approval |
| CF-075 | Queued completion mail first-request ordering | Conversational ordering contract approval |
| CF-080 | Subagent wait model (host-owned async wait vs bounded timeout polling) | Product decision on subagent coordination primitives |
| CF-081 | Detached review context scope (full history vs bounded diff context) | Code review agent quality benchmarking |
| CF-084 | Guardian parallel-wave context scope (pre-wave snapshot vs live sibling calls) | Security review on parallel tool approval semantics |
| CF-085 | Coalescing Guardian content-item boundaries | Measurement required on provider token impact |
| CF-086 | Guardian behavioral prose retained after removing schema restatement | Safety evaluation across Guardian providers |
| CF-091 | Provisional pre-start MCP catalog freshness policy | Product decision on startup latency vs tool freshness |
| CF-093 | Connector-specific MCP provenance representation | Prompt/security validation for per-tool attribution |
| CF-097 (unchanged-history) | Local compaction on unchanged non-empty history | Product decision on manual compaction idempotency |
| CF-098 | Local compaction summary source vs retained user suffix | Compaction summary quality evaluation |
| CF-103 | Aggregate retained tool-output eviction policy | Product decision and coding benchmark evaluation |
| CF-106 | MCP wall-time telemetry model visibility policy | Product decision on model-visible telemetry |
| CF-107 | Responses Lite catalog layout (append-only vs rebuild at item zero) | Provider caching benchmark and architecture review |
| CF-108 | Explicit prompt-cache breakpoint support | Provider feature compatibility evaluation |
| CF-110 | Service-tier continuation compatibility contract | Provider contract confirmation required |
| CF-112 | Continuation across schema-only V2 compaction changes | Provider protocol verification required |
| CF-116 (idempotency/contract) | Remote V2 compaction provider idempotency contract | Provider contract agreement or quarantine approval |
| CF-119 (provider commit contract) | Frameless partial-append replay risk; duplicate token commitment unproven | Service acknowledgement/idempotency contract |
| CF-122 | Provider-defined minimum remote-compaction tool catalog | Provider contract agreement specifying valid subsets |
| CF-126 | Provider usage reporting for legacy V1 compaction rollout accounting | Provider endpoint update or estimated charging approval |
| CF-127 | Durable task lifecycle metadata for coordination message retention | Product decision on multi-agent audit trail retention |

---

## Detailed Design Decision Records

### CF-001: Metadata Worker Context Profile and Bootstrap Inclusion

- **Sources & Facet:** #5 (worker-profile facet) (metadata-bootstrap facet)
- **Current Behavior & Token Implication:** Temporary metadata threads for title generation and session recaps currently inherit the full root-session bootstrap (`codex-rs/tui/src/temporary_structured_request.rs:46-164`, `codex-rs/app-server/src/request_processors/thread_processor.rs:1106-1456`), including project instructions, `AGENTS.md`, and World State fragments. This can consume 8K+ tokens on initial metadata turns.
- **Unresolved Choice & Tradeoffs:** Omitting project instructions and AGENTS.md saves tokens, but title quality and domain-specific terminology may degrade if the worker has zero repository context.
- **Proposed Direction & Options:**
1. Provide an explicit, minimal metadata-worker instruction template that omits repository files.
2. Pass a bounded 500-token project summary.
3. Keep existing bootstrap if user explicitly configures high-context metadata generation.
- **Approval & Gate Criteria:** Product decision required on whether metadata workers are allowed to omit project instructions and AGENTS.md, with output-quality benchmarking.

### CF-011: Budget-Aware Model Tool Catalog Planning (Direct vs Deferred)

- **Sources & Facet:** #1, H038
- **Current Behavior & Token Implication:** Codex either exposes tools directly in full or unselectively defers them to dynamic search (`codex-rs/core/src/tools/spec_plan.rs:496-530`, `codex-rs/core/src/mcp_tool_exposure.rs:84-98`). Small catalogs that easily fit are deferred unconditionally, while huge catalogs are exposed directly if search is disabled.
- **Unresolved Choice & Tradeoffs:** Direct exposure costs tokens on every request but provides zero-latency tool invocation. Deferred search saves prompt tokens but adds tool-search roundtrips, search latency, and search output context.
- **Proposed Direction & Options:**
1. Implement a unified catalog planner that exposes small/frequently used tools directly up to a fixed budget (e.g. 10K tokens) and defers the remainder to search.
2. Allow user/workspace configuration of direct tool priority.
- **Approval & Gate Criteria:** Product decision required on default tool budget thresholds and direct-vs-deferred prioritization heuristics.

### CF-016: MCP Audience and Priority Annotation Filtering Policy

- **Sources & Facet:** H063 (annotation-policy facet)
- **Current Behavior & Token Implication:** MCP `CallToolResult` items can carry audience (`user`, `assistant`) and priority annotations, but Codex treats all content items equally during model prompt assembly (`codex-rs/protocol/src/models.rs:2243-2384`).
- **Unresolved Choice & Tradeoffs:** Filtering `user`-only audience items from model prompts saves tokens, but third-party MCP servers may use annotations inconsistently or rely on the model seeing all output.
- **Proposed Direction & Options:**
1. Strictly filter out `audience: [user]` items from model context.
2. Use priority annotations to order items for truncation budgeting.
3. Treat annotations as advisory hints only.
- **Approval & Gate Criteria:** Product decision required on whether MCP audience and priority annotations constitute binding filtering authority.

### CF-020: Model-Independent Durable Media Representation and Reversibility

- **Sources & Facet:** H064 (durable-media-policy facet)
- **Current Behavior & Token Implication:** When tool outputs contain images or audio under a text-only model, media payloads are permanently converted or stripped before rollout persistence (`codex-rs/core/src/session/mod.rs:3065-3220`). Resuming the session on a multimodal model cannot restore the media without a re-fetch.
- **Unresolved Choice & Tradeoffs:** Persisting model-independent raw media payloads in the rollout store increases disk usage, but enables seamless cross-model resume without re-running tools.
- **Proposed Direction & Options:**
1. Persist canonical typed media in rollout storage and apply model-specific projections only at request construction time.
2. Keep irreversible text-only lowering for text models.
- **Approval & Gate Criteria:** Product decision required on storage overhead vs cross-model resume fidelity.

### CF-023: Automation Recurrence Prompt-Cache Affinity Keying

- **Sources & Facet:** H001
- **Current Behavior & Token Implication:** Scheduled automations spawn fresh sessions with new session IDs, causing `prompt_cache_key` to differ across recurrences (`codex-rs/core/src/session/session.rs:732-766`, `codex-rs/core/src/client.rs:514-525`).
- **Unresolved Choice & Tradeoffs:** Deriving cache keys from stable task identities increases prompt cache hit rates, but requires careful tenant/project namespacing to prevent cross-run state pollution.
- **Proposed Direction & Options:**
1. Add a namespaced `AutomationTaskIdentity` to derive deterministic prompt cache keys.
2. Keep random session-scoped cache keys.
- **Approval & Gate Criteria:** Product decision on cross-run cache isolation boundaries and provider caching compatibility.

### CF-024: Turn Idempotency Key Semantics and Protocol Contract

- **Sources & Facet:** H002
- **Current Behavior & Token Implication:** `clientUserMessageId` and response metadata are informational and not checked for deduplication before routing (`codex-rs/app-server-protocol/src/protocol/v2/turn.rs:153-173`, `codex-rs/core/src/session/turn_input.rs:264-318`). Duplicate submissions spawn separate turns.
- **Unresolved Choice & Tradeoffs:** Enforcing turn idempotency keys prevents duplicate model turns on network retries, but requires stateful key retention and defined conflict semantics.
- **Proposed Direction & Options:**
1. Introduce a strict `TurnIdempotencyKey` protocol contract in app-server v2 with a 24-hour deduplication window.
2. Retain current at-least-once submission semantics.
- **Approval & Gate Criteria:** API review and approval required for v2 turn submission idempotency contracts.

### CF-026: Automation Source Eligibility for Memory Extraction

- **Sources & Facet:** H004
- **Current Behavior & Token Implication:** Durable automation rollouts are currently eligible for background memory extraction passes (`codex-rs/state/src/runtime/memories.rs:218-244`), running extraction on repetitive cron jobs.
- **Unresolved Choice & Tradeoffs:** Extracting memories from scheduled jobs can capture background insights, but frequently extracts repetitive operational noise.
- **Proposed Direction & Options:**
1. Exclude automation threads from memory extraction by default.
2. Introduce an explicit `memory_mode` parameter in automation task definitions.
- **Approval & Gate Criteria:** Product decision required on whether automated cron tasks contribute to user/workspace memory consolidation.

### CF-029: Latest-State Reminder Projection vs Append-Only Audit Trail

- **Sources & Facet:** #30
- **Current Behavior & Token Implication:** Time reminders and rollout budget reminders append fresh user messages as time advances (`codex-rs/core/src/session/time_reminder.rs:79-136`). Older superseded reminder messages remain in history.
- **Unresolved Choice & Tradeoffs:** Replacing old reminders with latest state saves tokens, but breaks strict append-only history and prompt cache prefixes.
- **Proposed Direction & Options:**
1. Project reminders dynamically into turn context rather than persisting them as historical user messages.
2. Retain append-only historical reminders for cache stability.
- **Approval & Gate Criteria:** Product decision required on prompt cache hit rate vs context token savings for long-running sessions.

### CF-031: Goal Budget Authority Over Descendant Agents

- **Sources & Facet:** H052
- **Current Behavior & Token Implication:** Goal token budgets are enforced strictly within the parent thread (`codex-rs/ext/goal/src/extension.rs:97-115`), while subagents spawned by the goal draw from the shared rollout budget.
- **Unresolved Choice & Tradeoffs:** Propagating goal budget caps to subagents prevents runaway subagent token consumption, but requires hierarchical budget allocation protocols.
- **Proposed Direction & Options:**
1. Propagate goal budget limits hierarchically to all child agents.
2. Keep goal budgets thread-local.
- **Approval & Gate Criteria:** Product decision on multi-agent goal governance and budget hierarchies.

### CF-032: Active-Turn Semantics for Goal Mutation and Replacement

- **Sources & Facet:** H053
- **Current Behavior & Token Implication:** Mutating or replacing a goal while a turn is executing updates the active goal record but does not interrupt or steer the currently running inference (`codex-rs/ext/goal/src/api.rs:170-245`).
- **Unresolved Choice & Tradeoffs:** Interrupting active turns on goal mutation saves tokens on obsolete objectives, but may discard partially completed valid work.
- **Proposed Direction & Options:**
1. Automatically interrupt active turns when their governing goal is modified or cleared.
2. Allow current turn to finish before adopting new goal state.
- **Approval & Gate Criteria:** Product decision required on goal cancellation vs turn completion semantics.

### CF-034: Goal Accounting Persistence Failure Recovery Policy

- **Sources & Facet:** H055 (recovery policy facet) (persistence-failure policy facet)
- **Current Behavior & Token Implication:** When goal progress cannot be persisted, the system currently logs an error and continues.
- **Unresolved Choice & Tradeoffs:** Failing closed halts execution to save tokens and prevent unmetered loops, but degrades availability on transient database locks.
- **Proposed Direction & Options:**
1. Fail closed: immediately pause goal execution and report storage error.
2. Retry with backoff up to 3 times before pausing.
3. Fail open with user warning.
- **Approval & Gate Criteria:** Product approval required on fail-closed vs retry policies for goal accounting storage.

### CF-036: Goal Attachment Transport (Materialized References vs Typed Inline Objects)

- **Sources & Facet:** H110 (materialization facet)
- **Current Behavior & Token Implication:** Large goal attachments are materialized to temporary workspace files (`codex-rs/tui/src/goal_files.rs:33-137`), prompting the model to read them via tool calls.
- **Unresolved Choice & Tradeoffs:** Materializing files saves initial prompt tokens, but costs tool call roundtrips if the model must immediately read the file.
- **Proposed Direction & Options:**
1. Inline small attachments (< 2K tokens) directly into the goal prompt, materializing only large files.
2. Always materialize to files.
- **Approval & Gate Criteria:** Product decision required on inline threshold vs file reference materialization.

### CF-037: Memory Startup Scheduling (Once per Root Thread vs Per Turn)

- **Sources & Facet:** H098
- **Current Behavior & Token Implication:** Every qualifying turn start in app-server triggers a check to claim background memory extraction jobs (`codex-rs/app-server/src/request_processors/turn_processor.rs:673-684`).
- **Unresolved Choice & Tradeoffs:** Running checks on every turn drains the backlog faster, but adds repetitive SQLite queries and background task overhead.
- **Proposed Direction & Options:**
1. Schedule memory startup checks only on initial thread start and session idle transitions.
2. Keep per-turn polling with rate limits.
- **Approval & Gate Criteria:** Product decision on memory backlog processing cadence vs runtime overhead.

### CF-039: Condensing Phase 1 Extraction Rubric vs Recall Quality

- **Sources & Facet:** H099 (Phase 1 prompt facet)
- **Current Behavior & Token Implication:** Phase 1 extraction uses a comprehensive 7.7K-token rubric (`templates/memories/stage_one_system.md:1-120`) for every rollout extraction pass.
- **Unresolved Choice & Tradeoffs:** Condensing the prompt saves ~4K tokens per extraction call, but risks lower recall or lower precision in identifying durable user facts.
- **Proposed Direction & Options:**
1. Optimize and compress the rubric to ~2.5K tokens leveraging strict JSON output schema.
2. Keep verbose rubric for maximum extraction quality.
- **Approval & Gate Criteria:** Evaluation and benchmark approval required comparing compressed prompt extraction recall against baseline.

### CF-040: Memory Quota Lookup Failure Policy (Fail-Open vs Fail-Closed)

- **Sources & Facet:** H102
- **Current Behavior & Token Implication:** When rate limit or quota check fails or provider is non-Codex, `rate_limits_check` defaults to `unwrap_or(true)` (`codex-rs/memories/write/src/guard.rs:8-39`), allowing memory passes to proceed.
- **Unresolved Choice & Tradeoffs:** Failing open preserves functionality for custom providers, but risks exceeding quotas on backend outages.
- **Proposed Direction & Options:**
1. Fail closed for Codex backend errors, fail open with `NotApplicable` for third-party providers.
2. Always fail closed.
- **Approval & Gate Criteria:** Product decision required on quota failure handling policy across provider tiers.

### CF-042: Mode-Specific Phase 2 Consolidation Prompt Splitting

- **Sources & Facet:** H099 (Phase 2 prompt facet)
- **Current Behavior & Token Implication:** Phase 2 consolidation sends a monolithic prompt containing both INIT (cold start) and incremental consolidation instructions (`templates/memories/consolidation.md:116-190`), exceeding 12K tokens.
- **Unresolved Choice & Tradeoffs:** Splitting the prompt into INIT and incremental templates saves tokens, but requires maintaining separate prompt templates.
- **Proposed Direction & Options:**
1. Split into distinct `consolidation_init.md` and `consolidation_incremental.md` templates.
2. Keep monolithic template.
- **Approval & Gate Criteria:** Product approval required for prompt template split and quality verification.

### CF-045: Memory Extraction History Source (Raw Rollouts vs Post-Compaction History)

- **Sources & Facet:** H042
- **Current Behavior & Token Implication:** The direct reupload of checkpoint-retired discovery schemas is tracked in `BUGS.md`. The remaining policy question is whether other semantically meaningful pre-checkpoint task evidence should be mined from append-only rollout history or only from reconstructed active history.
- **Unresolved Choice & Tradeoffs:** Using active history minimizes repeated input; using raw rollouts preserves evidence that a compaction summary may have omitted. This decision must not be used to retain clearly nonsemantic schema catalogs already covered by the bug facet.
- **Proposed Direction & Options:**
1. Use reconstructed active history for all semantic extraction input.
2. Preserve append-only raw audit evidence but define a typed projection that excludes checkpoint-retired nonsemantic catalogs while retaining task evidence the compaction summary may omit.
- **Approval & Gate Criteria:** Product decision required on whether memory extraction operates on raw audit history or compacted active context.

### CF-049: Citation-Free Model History Projection vs Incremental Prefix Fidelity

- **Sources & Facet:** H103 (ordinary history facet)
- **Current Behavior & Token Implication:** The direct model-facing citation duplication is tracked in `BUGS.md`. Raw response items still serve rollout replay, raw-event compatibility, reparsing, and WebSocket prefix identity, so the unresolved architecture question is which derived projections are sanitized while the raw representation remains durable.
- **Unresolved Choice & Tradeoffs:** Rewriting the sole durable response risks provenance and continuation compatibility; request-time sanitized projections avoid token waste but require consistent history/`LastResponse` treatment.
- **Proposed Direction & Options:**
1. Keep the raw durable response and derive citation-free prompt, compaction, and Phase 1 projections with stable canonical IDs.
2. Rewrite the durable item itself and explicitly reset any continuation/cache state that depended on the raw prefix.
- **Approval & Gate Criteria:** Decision required on rollout representation fidelity vs prompt token savings.

### CF-051: Internal Worker Hook Profile (Exclusions for Review and Memory Workers)

- **Sources & Facet:** H005
- **Current Behavior & Token Implication:** Internal memory and review workers clone session configuration and execute registered hooks (`codex-rs/core/src/session/mod.rs:4361-4390`).
- **Unresolved Choice & Tradeoffs:** Disabling hooks for internal workers saves tokens, but prevents custom enterprise compliance hooks from auditing auxiliary workers.
- **Proposed Direction & Options:**
1. Define a restricted hook profile for internal workers that disables prompt-injection hooks while keeping audit hooks.
2. Keep all hooks enabled.
- **Approval & Gate Criteria:** Security and product decision on hook execution boundaries for internal sub-tasks.

### CF-053: Completed Stop Prompt Expiry at V2 Compaction Checkpoints

- **Sources & Facet:** H112
- **Current Behavior & Token Implication:** Remote V2 compaction retains historical `HookPrompt` items across compaction checkpoints (`codex-rs/core/src/compact_remote_v2.rs:493-565`).
- **Unresolved Choice & Tradeoffs:** Dropping completed Stop prompts at compaction saves context, but removes historical evidence of why the model stopped.
- **Proposed Direction & Options:**
1. Mark completed Stop prompts as ephemeral at compaction boundaries.
2. Retain Stop prompts permanently.
- **Approval & Gate Criteria:** Product decision required on whether Stop hook prompts represent permanent conversational history.

### CF-057: skills.read Completeness vs Total Resource Exposure Cap

- **Sources & Facet:** #13
- **Current Behavior & Token Implication:** `skills.read` allows reading entire multi-page skill documents through paginated calls under individual page limits (`codex-rs/ext/skills/src/tools/read.rs:70-77`).
- **Unresolved Choice & Tradeoffs:** Capping total exposure prevents massive skill files from consuming context, but may truncate complex skills.
- **Proposed Direction & Options:**
1. Impose an aggregate per-resource exposure cap (e.g. 50K tokens) with progressive summaries.
2. Allow unbounded multi-page reads.
- **Approval & Gate Criteria:** Product decision on maximum allowed skill resource size.

### CF-059: Turn-Scoped Skill Reactivation (References vs Full Bodies)

- **Sources & Facet:** H093 (cross-turn repetition facet)
- **Current Behavior & Token Implication:** When skills are re-mentioned across turns, their full instruction bodies are appended again (`codex-rs/core/src/session/turn.rs:773-930`).
- **Unresolved Choice & Tradeoffs:** Using lightweight references on re-activation saves tokens, but model attention on skill instructions may degrade on long conversations.
- **Proposed Direction & Options:**
1. Re-activate skills via compact reference tags if the full body is already present in prompt history.
2. Re-emit full body on every turn.
- **Approval & Gate Criteria:** Quality benchmarking required on model skill adherence with reference tags vs full instruction re-emission.

### CF-061: Eager vs Lazy Endpoint Plugin Recommendations

- **Sources & Facet:** H095 (recommendations facet)
- **Current Behavior & Token Implication:** Endpoint plugin recommendations are eagerly embedded in initial system context (`codex-rs/core/src/context/recommended_plugins_instructions.rs:6-53`).
- **Unresolved Choice & Tradeoffs:** Eager disclosure aids discoverability, but adds ~500 tokens to every request regardless of relevance.
- **Proposed Direction & Options:**
1. Provide recommendations lazily via tool discovery or intent matching.
2. Keep eager initial context injection.
- **Approval & Gate Criteria:** Product decision on plugin discovery UX vs initial prompt token overhead.

### CF-062: Query and Pagination API for Legacy Plugin Discovery

- **Sources & Facet:** H095 (legacy discovery facet)
- **Current Behavior & Token Implication:** `list_available_plugins_to_install` returns the entire sorted plugin candidate list without query filtering or pagination (`codex-rs/core/src/tools/handlers/list_available_plugins_to_install.rs:18-100`).
- **Unresolved Choice & Tradeoffs:** Redesigning the API to support query and cursor parameters requires protocol changes in client integrations.
- **Proposed Direction & Options:**
1. Update tool schema to support `query` and `limit` parameters.
2. Retain legacy dump behavior.
- **Approval & Gate Criteria:** App-server API schema review and versioning approval required.

### CF-063: Coalescing Authority-Specific Guidance in Skill Catalogs

- **Sources & Facet:** H096 (authority guidance facet) (authority-guidance facet)
- **Current Behavior & Token Implication:** Skill catalogs render separate authority guidance blocks for workspace, user, and system skills.
- **Unresolved Choice & Tradeoffs:** Coalescing guidance blocks saves tokens, but might blur security distinctions between trusted workspace and third-party skills.
- **Proposed Direction & Options:**
1. Unified guidance header with distinct authority labels on individual skills.
2. Retain separate authority blocks.
- **Approval & Gate Criteria:** Security review required to ensure unified headers preserve skill provenance clarity.

### CF-064: Unified Exec Wait Model (Host-Owned Async Wait vs Bounded Polling Yields)

- **Sources & Facet:** #4 (Unified Exec facet)
- **Current Behavior & Token Implication:** Unified Exec commands yield after 10 seconds, returning a process ID and requiring the model to poll via follow-up tool calls (`codex-rs/core/src/unified_exec/process_manager.rs:575-581`).
- **Unresolved Choice & Tradeoffs:** Host-owned async waits eliminate polling turns and save tokens, but reduce interactive responsiveness and model steering during long builds.
- **Proposed Direction & Options:**
1. Introduce host-managed async completion subscription for non-interactive commands.
2. Retain 10-second polling yields for interactive commands.
- **Approval & Gate Criteria:** Product decision required on async execution UX and timeout governance.

### CF-065: Code Mode Wait Model (Host-Owned Completion vs Bounded Timeout Yields)

- **Sources & Facet:** #4 (Code Mode facet)
- **Current Behavior & Token Implication:** Code Mode `wait` yields after 10 seconds (`codex-rs/core/src/tools/code_mode/wait_handler.rs:24-32`), requiring repeated model turns to await long cells.
- **Unresolved Choice & Tradeoffs:** Holding waits host-side until cell completion saves inference rounds, but blocks the turn loop.
- **Proposed Direction & Options:**
1. Allow configurable wait timeouts up to 300 seconds for background cells.
2. Keep 10-second yield limit.
- **Approval & Gate Criteria:** Runtime architecture approval required for long-lived Code Mode wait holds.

### CF-066: Terminal Output Normalization Policy (PTY Echo, Control Sequences, Screen Buffer)

- **Sources & Facet:** H008
- **Current Behavior & Token Implication:** Unified Exec forwards raw PTY bytes including terminal escape sequences and stdin echo directly to model text (`codex-rs/core/src/tools/context.rs:454-479`).
- **Unresolved Choice & Tradeoffs:** Stripping ANSI escape codes and terminal control sequences saves tokens and reduces noise, but may remove formatting or progress bars expected by downstream tools.
- **Proposed Direction & Options:**
1. Process PTY output through a headless terminal emulator (VT100 parser) to emit clean screen buffer text.
2. Strip common ANSI color/cursor sequences with regex.
3. Retain raw bytes.
- **Approval & Gate Criteria:** Product decision on terminal text fidelity vs prompt token reduction.

### CF-070: Hybrid Code Mode Schema Presentation (Dual Modality vs Exec Only)

- **Sources & Facet:** H026
- **Current Behavior & Token Implication:** In hybrid Code Mode, tools are exposed both as native JSON schemas for direct invocation and as TypeScript declarations in the `exec` environment (`codex-rs/core/src/tools/spec_plan.rs:540-552`).
- **Unresolved Choice & Tradeoffs:** Suppressing native JSON schemas for tools callable via TypeScript saves thousands of schema tokens, but restricts direct model tool invocation.
- **Proposed Direction & Options:**
1. Expose only the TypeScript environment definitions, routing all tool calls through `exec`.
2. Retain dual exposure for small tool sets.
- **Approval & Gate Criteria:** Product decision required on Code Mode invocation paradigms.

### CF-074: Agent Status API Response Format (Full Message Bodies vs Acknowledged References)

- **Sources & Facet:** #23
- **Current Behavior & Token Implication:** Subagent status APIs (`wait_agent`, `list_agents`) repeatedly return full completed message bodies in tool responses (`codex-rs/protocol/src/protocol.rs:1789-1804`).
- **Unresolved Choice & Tradeoffs:** Returning lightweight status handles and pagination references saves tokens, but requires the model to make explicit read requests for result content.
- **Proposed Direction & Options:**
1. Return status metadata with a content reference ID, requiring explicit `read_agent_result` only when needed.
2. Keep inlined message bodies for small completions.
- **Approval & Gate Criteria:** Subagent API v2 protocol design approval required.

### CF-075: Queued Completion Mail Admission (First User Turn vs Separate Turn)

- **Sources & Facet:** H022
- **Current Behavior & Token Implication:** The checked-in V2 contract queues completion mail with `trigger_turn = false`; a fresh user turn samples explicit user input first, and a non-final first response then causes a second request containing the already-available completion (`codex-rs/core/src/session/turn.rs:267,307-315,413-425`).
- **Unresolved Choice & Tradeoffs:** The extra request is established, but moving completion mail into the first request changes intentional user-versus-agent input ordering and final-answer deferral semantics. It remains a design decision until that ordering contract is approved.
- **Proposed Direction & Options:**
1. Merge pending agent completion mail into the initial prompt of the next user turn.
2. Keep distinct turn execution for agent mail.
- **Approval & Gate Criteria:** Conversational UX decision on multi-agent message ordering.

### CF-080: Subagent Wait Model (Host-Owned Async Wait vs Bounded Timeout Polling)

- **Sources & Facet:** #4 (agent wait facet)
- **Current Behavior & Token Implication:** `wait_agent` defaults to a 30-second timeout, returning a timeout status and prompting the model to poll again (`codex-rs/core/src/tools/handlers/multi_agents/wait.rs:102-201`).
- **Unresolved Choice & Tradeoffs:** Allowing completion-driven waits without timeouts saves polling rounds, but reduces caller responsiveness.
- **Proposed Direction & Options:**
1. Support `wait: true` with long timeouts (up to 5 minutes) backed by async notification events.
2. Retain 30-second polling cadence.
- **Approval & Gate Criteria:** Product decision on subagent coordination primitives.

### CF-081: Detached Review Context Scope (Full History vs Bounded Diff Context)

- **Sources & Facet:** #22
- **Current Behavior & Token Implication:** Detached review workers fork the entire parent conversational history (`codex-rs/app-server/src/request_processors/turn_processor.rs:1465-1477`).
- **Unresolved Choice & Tradeoffs:** Passing only the target git diff and relevant file context saves tens of thousands of tokens, but the reviewer loses conversational context on user intent.
- **Proposed Direction & Options:**
1. Provide a focused review context containing the diff, commit messages, and last user request.
2. Keep full parent history fork.
- **Approval & Gate Criteria:** Code review agent quality benchmarking required.

### CF-084: Guardian Parallel-Wave Context Scope (Pre-Wave Snapshot vs Live Sibling Calls)

- **Sources & Facet:** H070
- **Current Behavior & Token Implication:** Guardian reviews parallel tool executions sequentially, snapshotting history after preceding sibling calls have been persisted (`codex-rs/core/src/guardian/review_session.rs:509-645`).
- **Unresolved Choice & Tradeoffs:** Reviewing all parallel calls against a pre-wave snapshot saves token accumulation, but Guardian cannot inspect the combined impact of sibling mutations.
- **Proposed Direction & Options:**
1. Evaluate the entire parallel wave in a single batch Guardian request.
2. Use pre-wave history snapshot for all calls in the wave.
- **Approval & Gate Criteria:** Security review required on parallel tool approval semantics.

### CF-085: Coalescing Guardian Content-Item Boundaries

- **Sources & Facet:** H091
- **Current Behavior & Token Implication:** Guardian prompts render each section as a separate content item in the prompt vector (`codex-rs/core/src/guardian/prompt.rs:97-103`).
- **Unresolved Choice & Tradeoffs:** Coalescing into a single string saves minor JSON envelope overhead, but removes semantic section demarcation.
- **Proposed Direction & Options:**
1. Coalesce adjacent text sections into unified markdown blocks.
2. Retain separate items for structured inspection.
- **Approval & Gate Criteria:** Measurement required to verify provider token impact.

### CF-086: Guardian Prose Contract Reduction vs Non-Strict Schema Safety

- **Sources & Facet:** H092
- **Current Behavior & Token Implication:** The literal field/type/enum restatement is tracked as a direct bug facet in `BUGS.md`. The remaining prose also permits read-only investigation, defines the low-risk `{"outcome":"allow"}` shortcut, and requests fuller output for other cases—behavior not enforced by the non-strict schema.
- **Unresolved Choice & Tradeoffs:** Deleting the entire prose contract could reduce Guardian reliability. The decision is how much behavioral guidance remains after removing only the mechanically duplicated schema definition.
- **Proposed Direction & Options:**
1. For non-strict providers, retain investigation guidance and the low-risk shortcut after removing the duplicated property/type/enum definition.
2. For providers with strict structured output, evaluate removing the remaining format prose entirely.
- **Approval & Gate Criteria:** Safety evaluation across all supported Guardian provider models.

### CF-091: Provisional Pre-Start MCP Catalog Freshness Policy

- **Sources & Facet:** H031
- **Current Behavior & Token Implication:** Cached MCP tool definitions are exposed to the model before background server connection and verification complete (`codex-rs/codex-mcp/src/connection_manager/tool_catalog.rs:181-246`).
- **Unresolved Choice & Tradeoffs:** Exposing cached tools eliminates startup latency, but risks invoking tools that the server no longer supports.
- **Proposed Direction & Options:**
1. Block initial turn until MCP handshake completes if cached catalog is older than TTL.
2. Mark tools provisional and await verification.
- **Approval & Gate Criteria:** Product decision on startup latency vs tool catalog freshness.

### CF-093: MCP Plugin Provenance Namespace-Level Attribution

- **Sources & Facet:** H033
- **Current Behavior & Token Implication:** The exact regular-MCP server-scoped sentence repeated across common-membership children is tracked in `BUGS.md`. Codex Apps may have connector-specific plugin membership, so a universal namespace-only representation is not equivalent for every tool.
- **Unresolved Choice & Tradeoffs:** Namespace-level provenance is safe for regular servers with common membership; connector-specific app tools may still need per-tool attribution for correct selection and auditability.
- **Proposed Direction & Options:**
1. Render common regular-server provenance once at namespace level; for Codex Apps, retain per-tool attribution when connector membership differs.
2. Introduce connector-group provenance blocks if model-selection tests show they preserve attribution without per-tool repetition.
- **Approval & Gate Criteria:** Prompt engineering validation on plugin tool selection accuracy.

### CF-097: Local Compaction on Unchanged Non-Empty History

- **Sources & Facet:** H036 (unchanged history facet) (unchanged-history facet)
- **Current Behavior & Token Implication:** Manual local compaction re-summarizes history even if no new messages have been added since the last compaction.
- **Unresolved Choice & Tradeoffs:** Short-circuiting unchanged compaction saves tokens, but prevents users from requesting a fresh summary re-write.
- **Proposed Direction & Options:**
1. Return the existing compaction summary if history is unmodified.
2. Provide a `--force` flag for explicit re-summarization.
- **Approval & Gate Criteria:** Product decision on manual compaction idempotency.

### CF-098: Local Compaction Summary Source vs Retained User Suffix

- **Sources & Facet:** #29
- **Current Behavior & Token Implication:** Local compaction summarizes recent user messages and then also retains up to 20K tokens of those same user messages verbatim in the post-compaction history (`codex-rs/core/src/compact.rs:533-566`).
- **Unresolved Choice & Tradeoffs:** Excluding retained user messages from the summary input eliminates semantic overlap, but may omit important context if the summary is viewed in isolation.
- **Proposed Direction & Options:**
1. Exclude the retained user message suffix from the summarization input prompt.
2. Retain full history in summarization input.
- **Approval & Gate Criteria:** Compaction summary quality evaluation required.

### CF-103: Aggregate Retained Tool-Output Eviction Policy

- **Sources & Facet:** #2 (aggregate output facet)
- **Current Behavior & Token Implication:** Every executed tool output is retained in history until compaction occurs (`codex-rs/core/src/context_manager/history.rs:188-204`), allowing multiple outputs to accumulate to hundreds of thousands of tokens.
- **Unresolved Choice & Tradeoffs:** Evicting or aggressively summarizing older unique tool outputs saves massive context tokens, but the model loses access to previous tool execution results.
- **Proposed Direction & Options:**
1. Enforce an aggregate retained tool output budget (e.g. 50K tokens), replacing older tool outputs with compact summaries.
2. Retain all tool outputs until full session compaction.
- **Approval & Gate Criteria:** Product decision and coding benchmark evaluation on tool history retention limits.

### CF-106: MCP Wall-Time Telemetry Model Visibility Policy

- **Sources & Facet:** H065
- **Current Behavior & Token Implication:** MCP tool outputs include wall-time execution latency headers in the model-visible payload (`codex-rs/core/src/tools/context.rs:108-169`).
- **Unresolved Choice & Tradeoffs:** Stripping telemetry headers saves minor tokens, but models cannot reason about tool execution duration.
- **Proposed Direction & Options:**
1. Move execution latency to host-side event logs, omitting it from model prompt payloads.
2. Retain latency headers in tool outputs.
- **Approval & Gate Criteria:** Product decision on model-visible telemetry.

### CF-107: Responses Lite Catalog Layout (Append-Only vs Rebuild at Item Zero)

- **Sources & Facet:** #19
- **Current Behavior & Token Implication:** Responses Lite rebuilds the entire tool catalog as item zero of the request (`codex-rs/core/src/client.rs:885-931`), shifting all subsequent message indices and busting prompt cache prefixes on catalog changes.
- **Unresolved Choice & Tradeoffs:** Using an append-only catalog structure preserves prompt cache prefix stability, but requires provider support for dynamic tool declarations.
- **Proposed Direction & Options:**
1. Move tool declarations to a fixed append-only prefix or dedicated provider parameter.
2. Retain item zero catalog layout.
- **Approval & Gate Criteria:** Provider caching benchmark and architecture review required.

### CF-108: Explicit Prompt-Cache Breakpoint Support

- **Sources & Facet:** #20
- **Current Behavior & Token Implication:** Codex does not emit explicit prompt cache breakpoint markers (`codex-rs/codex-api/src/common.rs:303-382`), relying on provider automatic prefix caching.
- **Unresolved Choice & Tradeoffs:** Emitting explicit cache breakpoints maximizes cache hits for static system prompts, but requires provider API support.
- **Proposed Direction & Options:**
1. Add explicit `cache_control: { type: "ephemeral" }` annotations after system instructions for supporting providers.
2. Rely on automatic provider prefix caching.
- **Approval & Gate Criteria:** Provider feature compatibility and evaluation required.

### CF-110: Service-Tier Continuation Compatibility Contract

- **Sources & Facet:** H030
- **Current Behavior & Token Implication:** Switching service tiers (e.g. default to priority) currently invalidates `previous_response_id` continuation (`codex-rs/core/src/client.rs:330-384`), forcing a full context resubmission.
- **Unresolved Choice & Tradeoffs:** Reusing continuation across service tiers saves retransmission tokens, but requires provider confirmation that response IDs are shared across tiers.
- **Proposed Direction & Options:**
1. Allow continuation across tiers if provider confirms state sharing.
2. Force full resubmission on tier change.
- **Approval & Gate Criteria:** Provider contract confirmation required.

### CF-112: Continuation Across Schema-Only V2 Compaction Changes

- **Sources & Facet:** H114 (schema-only facet)
- **Current Behavior & Token Implication:** Remote V2 compaction clears the output schema, causing `responses_request_properties_match` to report mismatch and force a full connection reconnect.
- **Unresolved Choice & Tradeoffs:** Reusing the connection across schema-only compaction changes saves connection overhead, but requires provider support.
- **Proposed Direction & Options:**
1. Allow continuation when only output schema format changes during compaction.
2. Reset continuation across compaction.
- **Approval & Gate Criteria:** Provider protocol verification required.

### CF-116: Remote V2 Compaction Provider Idempotency Contract

- **Sources & Facet:** H113 (provider contract facet) (idempotency/contract facet)
- **Current Behavior & Token Implication:** Recovering from ambiguous transport failures during Remote V2 compaction risks duplicate operations without provider idempotency keys (`codex-rs/core/src/compact_remote_v2.rs:75-81`).
- **Unresolved Choice & Tradeoffs:** Requiring provider idempotency keys allows safe retries without token duplication, but requires provider API support or safe quarantine mechanisms.
- **Proposed Direction & Options:**
1. Introduce an idempotency key header for Remote V2 compaction requests.
2. In the absence of provider keys, quarantine ambiguous compactions without automatic retry.
- **Approval & Gate Criteria:** Provider contract agreement or product approval for quarantine policy required.

### CF-119: Frameless Realtime Reconnect Append Acknowledgements

- **Sources & Facet:** H044 (acknowledgement facet) (acknowledgement contract facet)
- **Current Behavior & Token Implication:** Frameless reconnect demonstrably resends the pending append from frame zero, but the repository has no service acknowledgement, committed offset, integration trace, or usage evidence proving that earlier successful sends were committed to model context before the later failure.
- **Unresolved Choice & Tradeoffs:** This is a provider commit/idempotency risk, not a demonstrated duplicate-token bug. Sequence numbers and acknowledgements enable exact resume but require a backend protocol contract; absent that contract, the client cannot distinguish retry from duplication.
- **Proposed Direction & Options:**
1. Adopt sequence-numbered context chunks with bidirectional acks in Frameless protocol.
2. Keep full buffer replay on reconnect.
- **Approval & Gate Criteria:** Keep outside `BUGS.md` until a service commitment contract, integration trace, or usage/transcript evidence proves prefix frames are counted twice. Realtime backend protocol approval is required for offset acknowledgements.

### CF-122: Provider-Defined Minimum Remote-Compaction Tool Catalog

- **Sources & Facet:** #17
- **Current Behavior & Token Implication:** Remote compaction requests transmit the complete tool catalog even though the compaction agent does not execute tools (`codex-rs/core/src/compact_remote_request.rs:62-89`).
- **Unresolved Choice & Tradeoffs:** Omitting unused tools saves catalog tokens, but the provider requires historical tool definitions to validate past tool call records.
- **Proposed Direction & Options:**
1. Obtain provider definition for minimal referenced-tool subsets and transmit only referenced schemas during compaction.
2. Retain full catalog until provider contract is specified.
- **Approval & Gate Criteria:** Provider contract agreement required specifying valid tool subsets for compaction.

### CF-126: Provider Usage Reporting for Legacy V1 Compaction Rollout Accounting

- **Sources & Facet:** H087
- **Current Behavior & Token Implication:** Legacy V1 compaction response exposes output items but omits usage token counts (`codex-rs/codex-api/src/endpoint/compact.rs:63-88`), so rollout token ledgers cannot account for compaction tokens.
- **Unresolved Choice & Tradeoffs:** Charging estimated tokens prevents budget leakage, but may disagree with actual provider billing.
- **Proposed Direction & Options:**
1. Update provider endpoint to return exact token usage.
2. Use local tokenizer estimation as approved fallback policy.
- **Approval & Gate Criteria:** Provider endpoint update or product approval for estimated budget charging required.

### CF-127: Durable Task Lifecycle Metadata for Coordination Message Retention

- **Sources & Facet:** H115
- **Current Behavior & Token Implication:** Remote V2 compaction retains all non-progress subagent coordination messages (`codex-rs/core/src/compact_remote_v2.rs:480-564`), accumulating historical negotiation messages.
- **Unresolved Choice & Tradeoffs:** Expiring completed task coordination messages at compaction saves context, but requires durable lifecycle metadata proving the task is terminal.
- **Proposed Direction & Options:**
1. Attach task lifecycle metadata and drop intermediate coordination messages once task is completed.
2. Retain coordination messages permanently.
- **Approval & Gate Criteria:** Product decision on multi-agent audit trail retention vs compaction token savings.
