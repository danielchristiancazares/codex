# Client-side fixes for excessive token burn

This records the September 8, 2026 assessment. Worktree status labels below
describe that inspection; subsequent commits may already implement the remedies.

## Scope

This assessment follows the [September 8 investigation](token-burn-investigation-2026-09-08.md).
The criterion is **fewer unnecessary input/output tokens for the same useful
work, model, reasoning effort, and service tier**. Spending caps, Goal wrap-up,
cheaper model selection, and reduced task scope are outside this fix list.

Source inspection covers the published `rust-v0.153.4` release and the current
fork worktree. Existing implementation changes belong to their ongoing
restoration stages. This assessment changes documentation only and reports
source tracing and public reproductions; it does not claim new runtime tests.

## September 2026 cancellation follow-up

The later cancellation sweep found that discarding an automatic recap's task
handle did not stop its model turn. Recaps now carry cancellation through the
temporary structured-request runner. Regaining focus cancels automatic recaps;
replacing the displayed thread or starting a new user turn cancels obsolete
recaps. A manual recap is not cancelled merely because focus returns.

Cancellation and cleanup have separate owners: the recap state signals
cancellation, while the worker remains alive long enough to interrupt the
temporary turn, unsubscribe, and deliver its stale completion for routing
cleanup. Cancellation before the worker's first poll must not submit a turn.
Thread startup is allowed to return its ID so a stale startup can unsubscribe;
aborting that future would lose the cleanup handle.

The same sweep reproduced an unintended `read_output` schema in otherwise
tool-less title and recap requests. `tools.read_output.enabled` now explicitly
controls that registration and defaults to `true`. Temporary metadata threads
set it to `false`; ordinary conversations retain output recovery, including when
their execution tools are otherwise unavailable. No model, effort, service tier,
or useful task scope is reduced by these fixes.

Regression coverage drives the TUI and embedded app-server with gated Responses
streams: it observes interruption, one original request, no cancelled pre-submit
request, and removal of temporary routing. The existing structured-recap test
checks the actual outbound request has no tools; the retained-output integration
test checks recovery still works without another command execution.

The Python async SDK also retains ownership of cancelled turn-start requests.
Requests still waiting for a same-thread or transport lock are withdrawn;
requests accepted during cancellation are interrupted by their exact returned
turn ID, and notification routes are released. Async overload backoff now waits
in the event loop, so cancelling it cannot leave a synchronous retry worker
submitting another request. Cleanup failures are logged rather than reported as
successful cancellation.

## Prioritized result

| Candidate | Mechanism that wastes work | Client-side remedy | Current fork state |
| --- | --- | --- | --- |
| Empty agent/process/cell polling | A timeout with no new information causes another model inference over the retained context. | Keep waiting in the runtime; wake for work results, user input, cancellation, or a genuine decision deadline. | Remaining. |
| Duplicate context publication | The same useful information occupies multiple prompt positions and is processed repeatedly. | Correct source-state tracking and retain one model-visible copy. | Several focused patches already present. |
| Terminal failure retries | A finished or malformed response is treated as a transient stream failure and sampled again. | Preserve terminal classification and end that attempt immediately. | Incomplete/protocol patches present; SSE `response.failed` still has a gap. |
| WebSocket liveness misclassification | A live delayed request is abandoned because control-frame activity is hidden from the reader's timeout. | Separate transport liveness from acknowledged-response progress. | Remaining. |
| Lost oversized tool results | Outer/history truncation discards evidence, prompting another execution/read to recover it. | Preserve queryable output before truncation and return a bounded recovery reference. | Artifact restoration planned; general execution-output retrieval absent. |
| Spurious compaction after a legacy filtered fork | Parent usage state is reused for a smaller child history, potentially triggering unnecessary compaction. | Recount the child's actual model-visible history. | Focused patch present. |
| Effort-change cache invalidation | Top-level reasoning settings change the prefix; old input becomes uncached at the transition. | Use supported positional configuration updates with a stable request baseline. | Gated history writer exists; complete cache-preserving behavior remains. |

“Patch present” means source changes exist in the dirty worktree. Validation and
completion belong to those stages. Savings depend on the triggering workload;
the issue reports do not support one percentage reduction across all users.

## 1. Eliminate empty model-mediated polling

### Concrete behavior

- V2 `wait_agent` defaults to 30 seconds and returns an ordinary tool result on
  an empty timeout.
- An omitted `write_stdin` yield starts at 250 ms and is clamped to **five
  seconds for an empty write**. The configurable maximum permits longer waits.
- Code Mode `wait` defaults to ten seconds.
- The generic tool-call path requests another sampling step. A quiet tool
  result therefore leads to another full logical-context inference.

Owners:
[V2 wait](../codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs),
[terminal polling](../codex-rs/core/src/unified_exec/process_manager.rs),
[Code Mode wait](../codex-rs/core/src/tools/code_mode/wait_handler.rs),
[follow-up selection](../codex-rs/core/src/stream_events_utils.rs).

The [measured report in #41875](https://github.com/openai/codex/issues/41875#issuecomment-5577095405)
attributes 7.13M parent input tokens to 47 empty polls. That is direct evidence
of unnecessary request volume even with a high cache-hit ratio.

### What to fix

Keep a wait operation pending while its dependency is still running and there
is no new information. UI progress can be updated through lifecycle events.
Deliver one tool result when there is a reason to resume inference.

Useful wake conditions are actual output/completion, a worker message requiring
delivery, user steering, cancellation, and an explicit deadline. Preserve all
real worker messages; their prose should not be heuristically classified and
discarded as “unimportant.” Structured heartbeat events can be coalesced.

For V2, existing `min_wait_timeout_ms` and `default_wait_timeout_ms` settings
already allow a measured longer-wait mitigation. Raising the minimum also
clamps model-authored explicit 30-second polls. That reduces polling frequency;
runtime suspension removes the timeout-only round trips themselves.

Implementation constraints:

- Preserve explicitly requested deadline behavior and interactive stdin writes.
- V2 already observes user steering; V1's wait implementation watches target
  status. Extend V1's wake handling before lengthening its waits substantially.
- A suspended wait must allow other tools needed by the dependency to run.
  The current generic runtime holds a parallel-execution read/write lock across
  handler execution; coordination waits require particular care.
- Subscribe/check state atomically so a completion arriving at admission is
  delivered. Preserve each call/output pair and cancellation outcome.
- Detect terminal dependencies and invalid handles promptly.
- Cache lifetime is a measurement variable. Model calls used solely to keep a
  cache warm would reintroduce the work being removed.

### Proof of improvement

Use a quiet child/process/cell with a controllable completion event. Count actual
Responses requests across the quiet interval. Verify prompt resumption once,
immediate user steering/cancellation, burst messages, process output, explicit
deadlines, and another tool that must run while the wait is pending.

## 2. Remove redundant model-visible context

These are concrete correctness fixes with a direct input-volume consequence.

### Additional-context omission accidentally forgets deduplication state

The committed `AdditionalContextStore::merge` replaces its remembered values
with every incoming map. The app-server adapter converts an omitted context
field to an empty map.

The triggering sequence is:

```text
publish A -> ordinary input omits additional context -> publish unchanged A
```

The middle operation forgets A, so the last operation injects it again while
the original A can still be in model history.

The worktree distinguishes keeping source state, publishing a replacement
snapshot, and explicit clearing. Finish that path through accepted/rejected
input, resume, rollback, and compaction.

Owners:
[source store](../codex-rs/core/src/state/additional_context.rs),
[turn adapter](../codex-rs/app-server/src/request_processors/turn_processor.rs).

### Review findings are inserted twice

The committed review-exit path records findings in the user-action review
envelope and then records their assistant presentation in model history.
The worktree retains the UI/event presentation while removing the second
prompt item.

Owner: [review task](../codex-rs/core/src/tasks/review.rs).

### Deferred tool schemas repeat

Repeated searches can append full definitions already visible in surviving
history. The worktree adds bounded, serialized-definition fingerprint tracking
and bounded search results. Validate that changed schemas and namespace metadata
remain visible, and that compaction permits rediscovery.

Owners:
[search handler](../codex-rs/core/src/tools/handlers/tool_search.rs),
[history-aware discovery](../codex-rs/core/src/context_manager/tool_discovery.rs).

### Proof of improvement

Inspect normalized outbound requests through the public turn API: one unchanged
context publication, one findings copy, and one surviving copy of each unchanged
schema. Include changed/cleared values and history reconstruction.

A redundant D-token fragment carried through R later requests adds roughly
D × R logical input tokens. Fixing publication also preserves the useful
content and task scope. Avoid rewriting old prompt items on each request just
to make them shorter: prefix stability must survive the fix.

## 3. Stop treating terminal responses as interrupted transports

### Incomplete and malformed terminal events

In 0.153.4, `response.incomplete` becomes a retryable stream error. An
output-limit termination can therefore cause another substantial generation.
Malformed completion handling can also fall into stream-recovery behavior.

The worktree introduces terminal incomplete/protocol failures. Complete and
validate that existing restoration rather than introducing another retry layer.

### A remaining SSE error-classification gap

The current SSE reader still saves most parsed `response.failed` errors and
waits for transport EOF. If the socket stays open, its timeout returns a new
generic stream error. A subsequent transport error can replace the parsed
terminal error as well.

That can turn a terminal, non-retryable server result into a retryable client
timeout. [#43140](https://github.com/openai/codex/issues/43140) supplies a
loopback reproduction of the error replacement.

**Narrow fix:** deliver the parsed terminal failure immediately and stop reading
that response. Retain its existing error code and retry delay. A
`response.failed` with missing error details can still retain the existing
retryable classification; immediate delivery and retry classification are
separate operations.

Owner: [Responses SSE adapter](../codex-rs/codex-api/src/sse/responses.rs).

The ordinary model client prefers WebSockets and supports production HTTP/SSE,
matching upstream: HTTP-only providers use SSE directly; HTTP 426 or exhaustion
of the WebSocket retry budget activates SSE for the rest of the session. This
retains the fork's request and telemetry metadata reuse, incremental requests,
WebSocket liveness tracking, and SSE terminal-failure preservation. The Guardian
classifier's separate HTTP path also benefits from correct SSE behavior.

### Proof of improvement

Use terminal events followed by an open stream, transport failure, and delayed
extra data. Assert the original error and actual inference-attempt count.
Include incomplete output-limit responses and malformed known terminal events.
Keep legitimate transient retries and retry delays working.

A saved failed request can represent little or substantial inference work.
Attempt reduction is directly testable; token savings require the corresponding
attempt's usage or a workload where generation had begun.

## 4. Avoid restarting viable, application-silent WebSocket responses

The WebSocket pump answers Ping and consumes Pong frames. The upper reader
applies its idle timeout to a message channel receiving neither kind of control
frame. Thus recent protocol activity cannot affect that timer.

Owner:
[WebSocket pump and response reader](../codex-rs/codex-api/src/endpoint/responses_websocket.rs).

[#43022](https://github.com/openai/codex/issues/43022) demonstrates this with the
installed 0.153.4 binary: a valid Pong arrived about 47 ms before the client
abandoned a delayed response. An application-level heartbeat allowed the same
delayed response to complete. The repository contains the matching path.

**Client fix:** track transport activity separately from response activity and
request acknowledgement. Keep a healthy, acknowledged delayed request attached
while applying a distinct bounded application-stall policy. Detect dead
transports promptly and preserve cancellation.

This addresses a specific source of repeated inference after long reasoning
intervals. Ping/Pong establishes connection liveness; application progress needs
its own treatment. Simply accepting Pongs forever or globally increasing every
timeout would leave important failure cases unresolved.

**Proof:** synthetic delayed completion with Ping/Pong, application heartbeat,
black-holed transport, and a permanently stalled application. Assert completion
on the original viable request and bounded handling of real failures.
The public reproduction establishes client abandonment; production token
attribution remains workload-specific.

## 5. Preserve oversized results so the agent can retrieve the missing evidence

The present layers have independent limits:

```text
raw collected result -> nested JavaScript result -> outer exec/wait result
                    -> retained history output
```

A model-selected large nested output can be truncated by the outer 10K-token
default and again by history. Once the original operation/cell is finished,
the missing portion often requires another tool invocation to recover.
[#40897](https://github.com/openai/codex/issues/40897) and
[#43229](https://github.com/openai/codex/issues/43229) contain observed
truncation/reread patterns and growing per-request input.

Owners:
[nested/direct output](../codex-rs/core/src/tools/context.rs),
[outer output](../codex-rs/core/src/tools/code_mode/mod.rs),
[process capture](../codex-rs/core/src/unified_exec/process_manager.rs).

**Client fix:** preserve bounded, queryable captured output before generating the
inline preview. Return a reference and loss/completeness information with that
preview. Support targeted ranges/search so an omitted line can be recovered
without rerunning the command or printing the whole batch again.

Keep full-data processing available inside JavaScript. Code Mode can be highly
efficient when it processes a large raw result and emits a small answer.
Clamping all nested data to the model-visible preview size could break that
behavior and increase rereads.

The runtime already exposes session `store`/`load` helpers, which can preserve
explicitly stored results across cells. Automatic bounded output recovery still
needs implementation. The existing `restore-exec-artifacts` stage in `plan.md`
already owns broader execution capture/query restoration; reuse it.

Coordinate final preview and history allowances, including framing, so useful
evidence and its retrieval reference survive the final outbound representation.

**Proof:** place required evidence in the omitted middle of several large
results. Recover it through a bounded query and assert that the original command
ran once. Preserve JavaScript computations over the full captured data and
accurately report partial capture, expiry, and scope.

## 6. Eliminate unnecessary compaction caused by stale fork accounting

Filtered agent forks can retain substantially less history than their parent.
The committed non-paginated fork/reconstruction path can carry the parent's
token-count state into that smaller child. The paginated path already removes
those events. On the affected path, pre-turn context checks can compact the child
using an inherited estimate that describes a different prompt.

The worktree excludes inherited usage snapshots where inappropriate and recounts
the reconstructed child's model-visible history.

Owners:
[fork filtering](../codex-rs/core/src/agent/control/spawn.rs),
[recounting](../codex-rs/core/src/session/token_recount.rs),
[pre-turn compaction](../codex-rs/core/src/session/turn.rs).

**Proof:** fork a small selected portion from a legacy parent near its context limit.
Assert that the child's first request uses the smaller history and proceeds
without a needless compaction inference. Keep genuine context exhaustion and
ordinary resume accounting correct.

The local compaction/request-fit restoration also bounds repeated reductions.
Treat rejected-request traffic separately from generated summary tokens when
measuring that work's savings.

## 7. Preserve cache through supported reasoning-effort transitions

The current client builds request-level `reasoning.effort` from the selected
step setting. Its incremental reuse comparison includes that object.
The gated history writer also appends configuration updates, while the existing
integration test explicitly expects the request effort to change from medium
to high to low. Enabling that flag alone leaves the cache-changing behavior.

Owners:
[request construction](../codex-rs/core/src/client.rs),
[configuration writer](../codex-rs/core/src/session/reasoning_effort.rs),
[existing integration contract](../codex-rs/core/tests/suite/reasoning_effort_override.rs).

**Client fix:** on a supported request path, keep a stable request-level baseline
and carry the selected effective effort in trusted positional
`configuration_update` items. Preserve input order, update placement, and
history lineage through rollback, fork, resume, and compaction.

The [current public API contract](https://developers.openai.com/api/docs/guides/reasoning#change-reasoning-mid-conversation)
has concrete restrictions:

- GPT-6 Astra, standard single-agent mode.
- Updates placed between responses, before the next user message.
- Adjacent configuration updates are rejected.
- Automatic compaction/truncation and the standalone `/responses/compact`
  endpoint cannot be combined with these updates.
- Explicit `compaction_trigger` is supported, followed by a fresh desired-effort
  update before the next user message.

Codex's local agent tree and the API's multi-agent mode are different surfaces;
verify the actual backend request mode rather than inferring compatibility from
the presence of local workers. `use_responses_lite` alone is insufficient proof.
This is a client implementation against an existing supported API, with a
capability/compaction boundary, rather than a blanket model-family toggle.

#43229 reports transition cache-hit drops from approximately 99% to 0–12%,
followed by recovery. This fix targets the uncached-input spike at transitions.
That report's 98.32% overall hit rate limits the share attributable to this
mechanism.

**Proof:** assert both the stable top-level setting and correct positional
effective settings. Then measure backend cache-write, cache-read, and ordinary
input counts for a matched transition. A stable cache key by itself is
insufficient.

## Measurement and execution order

Retain the chosen model/effort/tier and the same task acceptance criteria. For
each candidate compare:

- Useful task completion and correctness.
- Responses attempts, including those interrupted after acknowledgement.
- Empty-wait-only sampling steps.
- Total input, cached input, cache-write input, output, and reasoning output.
- Duplicate fragments in normalized outbound input.
- Original command executions versus bounded output-retrieval queries.
- Compaction invocations, their generated tokens, and post-compaction rereads.

Use per-response usage records where available, deduplicated by response ID.
Track incomplete/failed attempts separately. Counting cumulative snapshots or
transport bytes as billed tokens would confound the comparison.

Recommended order:

1. Finish existing terminal-error, duplicate-context, and filtered-fork fixes;
   add the remaining SSE terminal-failure correction.
2. Remove empty polling across agents, terminal processes, and Code Mode.
3. Correct the WebSocket liveness/progress distinction.
4. Complete recoverable output and eliminate repeated truncation.
5. Implement capability-safe, cache-preserving effort transitions.

Guardian's extra request count and memory's startup work remain diagnostic
candidates until a specific redundant invocation or repeated-prefix defect is
measured. Their mere existence is insufficient evidence of an efficiency bug.
Quota weighting and entitlement reconciliation remain backend responsibilities.
