# Token-burn investigation — September 8, 2026

Worktree status labels record the state inspected on this date. Subsequent
commits may already address the client behavior described below.

The follow-up [client-side fix assessment](token-burn-client-fixes.md) narrows
remediation to avoidable token work for the same task. Goal-budget accounting
below is an observability distinction, rather than a cause of accelerated burn.

## Scope and conclusion

This is an engineering investigation for the fork. It combines source inspection
with public issue telemetry. The strongest client-side mechanism is **repeated
model inference over large, mostly cached contexts**, especially empty agent
polling. Goal budgets currently exclude cached input, making them a poor proxy
for the allowance consumed by that workload. Tool-output accumulation, extra
reviewer requests, reasoning-setting changes, and terminal-response retries add
other amplification paths.

The Astra-specific increase also warrants server-side reconciliation. The client
receives quota percentages from OpenAI; the source available here does not
establish the account entitlement or the conversion from a model's usage to
included subscription allowance. These are separate quantities:

- Effective input processed by inference, including cached input.
- Transport bytes, which can be reduced by WebSocket incremental requests.
- Uncached input and output counted against a Goal token budget.
- Subscription quota or purchased credits deducted by the backend.

High cache hit rates can coexist with very large absolute usage. A full-history
transport resend also does not, by itself, establish a provider cache miss.

### Revisions and method

- Reported CLI release: `rust-v0.153.4`,
  `3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`, published September 4.
- Fork's recorded upstream base:
  `b4373e53ab79df7baadc6805dea54a060b820307`, September 7.
- Local HEAD: `2a1b16ea7fd2b30938d23fda4371653399e67b0c`.
- Existing staged state was empty. The worktree contained extensive implementation
  changes, including token/context fixes. Those changes were preserved.
- The published release was fetched and its source extracted to a temporary
  directory for comparison. Source links below target that release.
- Validation comprised tracing request construction, continuation, truncation,
  retry, budgeting, auxiliary inference, and reporting paths; comparing release
  behavior with the worktree; and checking the arithmetic in the examples.
- Public telemetry is attributed to its reporters. Their private diagnostics and
  OpenAI's billing ledger were unavailable. No live inference experiments or
  Rust test runs were performed. This investigation adds only this document.

## What the reports actually establish

The five initial tickets report five-hour or weekly allowance depletion, rather
than measured monthly token totals.

| Report | Useful observation | Evidence limit |
| --- | --- | --- |
| [#43967](https://github.com/openai/codex/issues/43967) | Pro 20x, Astra Ultra, two Goals totaling about 80 minutes, approximately 50 weekly percentage points consumed. | Before/after UI observation; request and worker usage require submitted diagnostics. |
| [#43955](https://github.com/openai/codex/issues/43955) | CLI 0.153.4, Astra xhigh, three concurrent threads, weekly remaining 80% to 7% in under an hour. | Concurrency is explicit; subagent activity is a question in the report. |
| [#43904](https://github.com/openai/codex/issues/43904) | Plus-like plan, Windows app, approximately ten minutes to exhaust a five-hour window. | Model and request counts missing. |
| [#43922](https://github.com/openai/codex/issues/43922) | App 26.901.51231, $20 plan, feedback submitted. | Essentially no workload or numerical evidence. |
| [#43720](https://github.com/openai/codex/issues/43720) | Plus, Windows app, Astra, reported three-minute exhaustion. | Model/effort description is inconsistent; comments also mention other models. |

The wider issue graph supplies much better leads:

- [#41875, measured CLI reproduction](https://github.com/openai/codex/issues/41875#issuecomment-5577095405):
  47 empty 30-second parent polls, 7,130,181 input tokens, 7,114,112 cached.
  The overall run included two workers and moved the five-hour meter from 53%
  used to its cap. The entire meter movement cannot be attributed to polling.
- [#43229](https://github.com/openai/codex/issues/43229): 4,976 unique successful
  response records, 673.46M input, 655.16M cached input, and 2.15M output in a
  local-day audit. It records large nested outputs, truncation, and three
  effort-change cache misses. Its meter and token aggregation intervals differ.
- [#40897](https://github.com/openai/codex/issues/40897): 52 usage events in
  approximately 16 minutes, context increasing from about 21K to 194K, repeated
  parallel-output truncation and rereads. This predates Astra.
- [#42996](https://github.com/openai/codex/issues/42996): a source-level analysis
  of request-level effort changes, corroborated by the transition measurements
  in #43229.
- [#43287](https://github.com/openai/codex/issues/43287): 58 completed
  `codex-auto-review` sampling steps alongside 150 Sol steps in about two hours.
  The report does not establish whether those reviewer requests were charged.
- [#43222](https://github.com/openai/codex/issues/43222): same-account
  Astra/Sol comparisons and quota-bucket anomalies. Some datasets use
  `token_count` snapshots and concurrent or partially observed workloads;
  these support reconciliation, not an exact inferred quota multiplier.
- [#42987](https://github.com/openai/codex/issues/42987): short Medium-effort
  workflows, including a comment reporting a tool-free interaction. This keeps
  model weighting/accounting in scope alongside orchestration overhead.
- [#41220](https://github.com/openai/codex/issues/41220): a useful cross-report
  index spanning older model families, background activity, accounting, and
  workflow amplification. Duplicate-bot links are discovery aids, not causal
  confirmation.

## Findings

### 1. Empty agent waits cause another model step

**Priority: high. Confidence: source-confirmed, with matching public telemetry.
Still present in this worktree.**

V2 defaults to a 30-second wait and allows a ten-second minimum. Its handler
returns an ordinary successful tool output containing `"Wait timed out."` and
`timed_out: true`. The generic tool-call path marks the response as requiring
follow-up; the turn loop samples again. It has no distinct runtime suspension
path for a timeout that brought no new information.

Sources:
[defaults](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/config/mod.rs#L234),
[wait result](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs#L147),
[tool follow-up](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/stream_events_utils.rs#L327).

At 150K effective input tokens, 30-second polling can account for **18M input
tokens per hour**. Five-minute polling would account for 1.8M under the same
constant-context assumption. These are input-volume examples, excluding model
latency and output; they are not quota-price estimates.

The wait already wakes on mailbox activity or user steering. Longer defaults
can preserve responsiveness to those events. Explicitly passing `30000`
overrides the default, so changing only the default will leave some workloads
unchanged.

**Remediation:** introduce an event-driven waiting consequence that keeps the
model suspended while nothing actionable changes; preserve explicitly requested
timeout semantics. Coalesce non-actionable mailbox updates. A longer default
and aligned tool guidance are a smaller first stage. Audit `write_stdin` and
Code Mode wait loops for the same no-output polling pattern.

### 2. Accounting observation: Goal budgets exclude cached input

**Classification: accounting context. Confidence: source-confirmed.**

`goal_token_delta_for_usage` computes:

```text
input_tokens - cached_input_tokens + output_tokens
```

Descendant accounting uses the same function. A Goal therefore charges zero
budget units for cached input. The 7.13M-input polling example above would add
only **16,069 input tokens** to this budget, plus its output tokens.

Sources:
[budget arithmetic](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/ext/goal/src/accounting.rs#L441),
[descendants](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/ext/goal/src/accounting.rs#L200),
[budget defaulting](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/ext/goal/src/tool.rs#L199),
[automatic continuation](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/ext/goal/src/runtime.rs#L399).

When an explicit budget and configured maximum are both omitted, a new Goal
has no token ceiling. Active Goals automatically start another turn when idle.
The code does stop Goals on terminal turn errors and usage-limit failures;
the normal continuation policy remains driven by Goal state.

This explains why the Goal counter differs from processed-token volume. Changing
the counter or wrap-up policy would change spending control, without removing the
unnecessary inference identified in finding 1. It is therefore outside the
accelerated-burn fix list.

### 3. Nested output budgets can produce expensive truncation/reread cycles

**Priority: high. Confidence: source-confirmed mechanism, observed workload
amplification. Still present.**

Nested `exec_command` results use the supplied `max_output_tokens`; omitted
nested limits expose the raw collected output to the JavaScript cell. The outer
Code Mode result has a separate, default 10K-token budget. Its resolver accepts
the requested value without an aggregate policy cap at that layer. History then
applies its own per-output truncation policy.

Sources:
[nested result](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/tools/context.rs#L414),
[outer truncation](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/tools/code_mode/mod.rs#L326),
[budget resolver](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/unified_exec/mod.rs#L219),
[history cap](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/context_manager/history.rs#L261).

A batch of several 30K–60K nested results can therefore be truncated at the
outer boundary and again when retained. The whole raw batch is not necessarily
model input. The waste comes from lost useful evidence, additional retrieval,
and the repeated inference cost of the excerpts that do survive.

For scale, 50 requests starting with a 20K prefix and adding 4K retained tokens
between requests produce 5.9M cumulative input tokens. That requires neither a
cache failure nor a large final code diff.

The bundled 272K window ordinarily
[auto-compacts at 90%](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/protocol/src/openai_models.rs#L499),
or 244,800 tokens.
A reported 194K input remaining uncompacted is consistent with that threshold.
Experimental context-management modes and remote catalog overrides can select
different behavior.

**Remediation:** coordinate nested and outer result budgets, retain full outputs
as recoverable artifacts, and return bounded excerpts/references. Preserve
tool-result identity and append-only history. Merely shrinking the compaction
threshold adds summary calls and cache rebuilding; measure the whole workflow.

### 4. Reasoning-effort changes invalidate incremental request reuse

**Priority: medium. Confidence: source-confirmed request change, matching
reported cache misses. Still present.**

The released sampling path passes the current selected effort into the client.
`build_reasoning` writes it into request-level `reasoning.effort`.
Incremental reuse requires equality of that reasoning object.

Sources:
[selected effort](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/session/turn.rs#L2264),
[request reasoning](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/client.rs#L906),
[reuse comparison](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/client.rs#L397).

The fork's newer upstream base contains a gated `reasoning_effort_override`
writer for trusted `configuration_update` history items. The flag defaults off,
and request construction still follows the current selected effort. Its presence
alone does not complete cache-preserving transitions.

In #43229, three transitions changed cache ratios from approximately 99% to
12.05%, 0%, and 12.06%; the next requests recovered. That report's overall cache
ratio was 98.32%, limiting this mechanism's explanatory share.

**Remediation:** for explicitly supported models/providers, keep a stable
request-level effort baseline and append trusted positional updates. Cover
resume, compaction, rollback, forks, model changes, and unsupported providers.
Use request-body assertions first; backend telemetry establishes actual cache
savings.

### 5. Terminal incomplete responses enter the generic retry path

**Priority: high for released code. Confidence: source-confirmed.
Already addressed by existing worktree changes; those changes were not authored
or independently runtime-validated by this investigation.**

The release turns every `response.incomplete` into `ApiError::Stream`, retaining
only its reason text. That category is retryable. Its accompanying usage is
discarded instead of reaching normal completion accounting.

Sources:
[incomplete classification](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/codex-api/src/sse/responses.rs#L468),
[retry state machine](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/responses_retry.rs#L85),
[retry defaults](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/model-provider-info/src/lib.rs#L28).

Five stream retries permit six sampling attempts. WebSocket-to-HTTP fallback
resets that counter, permitting another six when the fallback path is available.
Some terminal reasons, such as exhausted output limits, can involve substantial
generation before the client retries.

This establishes a potentially costly and locally underreported failure path.
The five initial reports contain insufficient attempt-level evidence to measure
its contribution.

The worktree introduces concrete incomplete/protocol failures, preserves
incomplete usage, and changes transport policy. Preserve that work and validate
it through its existing implementation stage. Genuine connection-establishment
retries have a separate policy; their existence alone does not demonstrate
repeated inference.

### 6. Auxiliary model activity needs separate usage attribution

**Priority: medium. Confidence: source-confirmed additional inference;
quota charge depends on effective configuration and backend treatment.**

Guardian can issue both approval-review requests and asynchronous Luna
classification requests. Its `free_guardian` configuration selects dedicated
unmetered routes for eligible Codex-backend requests; the local default is false.
Other configurations use the normal Responses endpoint.

Sources:
[flag resolution](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/config/mod.rs#L1508),
[review routing](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/client.rs#L1084),
[classifier routing](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/ext/guardian-v2/src/async_scorer/sampler.rs#L238).

Reviewer transcripts have per-entry and aggregate retention bounds, yet their
requests can still be substantial and frequent. Reused review conversations
accumulate additional exchanges. The worktree's existing aggregate prompt-bound
changes address part of this surface.

When memories are enabled, root-session startup launches asynchronous extraction
and consolidation. The defaults allow two eligible rollouts per startup and
perform a best-effort check for 25% remaining quota; default extraction/
consolidation models are Luna/Terra. **A failed quota lookup permits the work to
proceed**, through `unwrap_or(true)`. The check occurs before the two-stage
pipeline. This is bounded, conditional background work that can contribute usage
while the foreground appears quiet. The memory feature defaults off in the
open-source feature table.

Sources:
[memory startup](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/memories/write/src/start.rs#L20),
[memory limits](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/config/src/types.rs#L48),
[quota-check fallback](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/memories/write/src/guard.rs#L9).

**Remediation:** attribute each auxiliary request to its parent, purpose, endpoint,
and observed usage; distinguish unmetered routes. Review duplicate assessments
and retain required approval semantics. Consider deferring opportunistic memory
work after a failed ChatGPT quota lookup and rechecking between stages.
Desktop-specific scheduling outside this repository needs host-side diagnostics.

### 7. Defaults, Ultra, and fork inheritance multiply the workload

**Priority: high diagnostic relevance. Confidence: source-confirmed behavior.
These are product policies rather than an established accounting defect.**

The [0.153.4 release](https://github.com/openai/codex/releases/tag/rust-v0.153.4)
explicitly made Astra the bundled default when a model is not configured.
The bundled effort remains low; upgrading alone does not select Ultra.
Remote catalogs can override bundled defaults.

Ultra selects proactive delegation in V2. Ordinary built-in non-Ultra policy
requires an explicit request, subject to configured/catalog policy overrides.
V2 `spawn_agent` defaults to `fork_turns="all"` and ordinarily inherits the
parent model/effort. Forking still applies history filtering; it should not be
described as blindly copying every parent tool result.

Sources:
[delegation policy](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/session/multi_agents.rs#L155),
[fork default](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs#L298),
[inheritance guidance](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/core/src/session/multi_agents.rs#L50).

V2 has per-session execution/residency controls. They constrain concurrency,
while repeated or nested work within those slots still consumes usage.
Independent top-level sessions add their workloads together. The three-thread
report and the Ultra Goal report therefore have different multiplication paths.

**Remediation:** expose the resolved model, effort, tier, delegation policy, and
parent/worker usage together. Use explicit bounded handoffs and appropriate
worker models where authorized. Measure task completion and total usage before
changing defaults.

### 8. Additional context duplication is already being addressed in the fork

**Confidence: source-confirmed release surfaces; existing worktree remediation.**

The inspected worktree already contains focused changes for:

- Repeated tool-search schema insertion and excessive search-result counts.
- Single-copy review findings in model history.
- Bounded successful subagent completion envelopes.
- Additional-context source-state preservation and deduplication.
- Fork token recounting, final request fitting, and progress-making compaction.

These are relevant because one unnecessarily retained fragment is processed on
every subsequent request until its history window ends. They belong to the
existing restoration stages in `plan.md`. This audit does not claim their tests
passed or treat them as newly implemented findings.

## Reporting gaps and interpretation

Use `token_usage_record.usage` deduplicated by response ID where available.
`token_count` is a state snapshot: rate-limit-only updates can repeat previous
usage, resume can restore it, and context-window recovery can substitute an
estimate. Summing every `last_token_usage` or every cumulative total can inflate
an audit. Include long-lived rollouts modified during the interval and all
participating clients, rather than only files created that day.

Sources:
[per-response record contract](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/protocol/src/protocol.rs#L2239),
[quota refresh source](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/backend-client/src/client.rs#L337).

Cached input is a subset of input, and reasoning output is a subset of output
on this Responses path. Keep them separate without adding either twice.
Successful response records also leave gaps for incomplete/failed requests and
some auxiliary calls. Per-request effort/tier can change within a turn, so a
single startup setting is insufficient attribution.

The missing 25% warning in #43955 is compatible with UI behavior: warnings
coalesce to the highest threshold crossed by a snapshot, and a 100%-used snapshot
suppresses them. That symptom alone cannot establish a sudden billing adjustment.

## Smallest useful follow-up stages

1. **Stop empty-poll amplification.** Add a deterministic wait-until-activity
   path and request-count integration coverage with a quiet worker, a real
   completion, intermediate mailbox activity, and user steering. Measure empty
   model reentries per child completion.
2. **Remove duplicate model-visible content.** Verify single-copy review findings,
   unchanged additional-context publication across omitted updates, and
   history-aware tool-schema deduplication.
3. **Finish the existing retry/context restoration stages.** Verify terminal
   incomplete handling, actual normalized outbound request bounds, schema
   uniqueness, and progress-making compaction. Keep task ownership intact.
4. **Coordinate output budgets and recovery.** Exercise several large nested
   results with useful data at different positions; verify recoverable artifacts,
   bounded retained output, and reduced retrieval/request counts.
5. **Complete supported effort-update semantics.** Assert a stable request
   baseline and correctly positioned trusted updates across lifecycle changes.
6. **Build a reconcilable per-attempt usage view.** Capture response/request ID,
   parent/root, trigger (user, tool follow-up, timeout, Goal, review, memory,
   compaction, retry), actual model/effort/tier, terminal outcome, input/cached/
   output/reasoning usage, effective context size, and retry lineage.

For OpenAI reconciliation, correlate that request timeline with the exact
quota-bucket ID, window/reset timestamps, effective Pro tier, remote feature
configuration, charged units, and metering adjustments. The highest-value cases
are the measured polling reproduction, the unique-response ledger in #43229,
and matched Astra/Sol windows with all concurrent clients accounted for.

**Working diagnosis:** the client demonstrably permits expensive repeated work
through empty polling, redundant prompt content, and retry amplification.
Those are client-side efficiency targets independently of the unresolved Astra
subscription-weighting and entitlement questions.
