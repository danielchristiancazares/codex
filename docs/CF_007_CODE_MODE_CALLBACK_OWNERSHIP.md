# Code Mode Callback Ownership Reference Architecture

**Scope:** Prevent delayed Code Mode notifications and nested tool callbacks from entering a later
turn after the turn that created their cell has ended.
**Security Classification:** Low
**Audience:** Rust developers extending Code Mode dispatch and session-turn lifecycle behavior
**Prerequisites:** Familiarity with `codex-core` sessions, `TurnContext`, Code Mode cells, and
Tokio cancellation.
**Related Documentation:**
- [Codex core](../codex-rs/core/README.md)
- [Token-burning bug record](../BUGS.md#cf-007-code-mode-callbacks-route-to-stale-active-turns-via-shared-broker)

Code Mode executes cells in a shared runtime while Codex turns come and go. A yielded cell may
produce a notification or request a nested tool after its initiating turn has finished. The
dispatch broker therefore treats the initiating turn and its worker generation as an unforgeable
ownership boundary: callbacks are accepted only while that exact owner is active, and every
execution path revalidates ownership immediately before model-visible work occurs.

**Normative Language:** The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD
NOT, RECOMMENDED, MAY, and OPTIONAL are to be interpreted as described in RFC 2119 and RFC 8174.

## 1. Overview

`CodeModeDispatchBroker` maps runtime callbacks to per-turn dispatch workers. A cell is registered
against the turn that executed it; the worker invokes nested tools and injects notifications into
that same session turn. The existing design records a turn ID and checks it before queueing, but a
queued message or spawned invocation can survive the check and run after the owner has ended
([`delegate.rs:135-233`](../codex-rs/core/src/tools/code_mode/delegate.rs#L135-L233)).

The architecture adds an explicit worker generation to cell ownership and validates the complete
owner tuple at dispatch and at host execution. A callback whose cell is closed, whose worker has
ended, or whose session no longer has the matching active turn is rejected and produces no
model-visible item.

| Decision | Approach | Rationale |
|---|---|---|
| Ownership identity | `(session identity, turn_id, worker generation)` | Turn IDs alone do not describe the lifetime of a worker instance. |
| Enforcement point | Before queueing and immediately before `submit_tool` / `inject_if_turn_running` | Closes both the admission race and the spawned-task lifetime race. |
| Stale callback behavior | Return a cancellation/owner-ended error and drop output | Stale output has no valid consumer and must not be replayed into another turn. |
| Interrupt scope | Terminate cells whose complete owner matches the interrupted turn | Preserves the existing per-turn cancellation intent. |

## 2. Functional and Non-Functional Requirements (NFRs)

### Functional Requirements

- **REQ-1:** Every ready cell MUST have exactly one immutable owner capability.
- **REQ-2:** A callback MUST be dispatched only to the worker identified by its cell owner.
- **REQ-3:** A callback MUST be rejected if the cell is closed, the worker generation is gone, or
  the owner turn is no longer active.
- **REQ-4:** A notification MUST be injected only through the identified-turn atomic session API.
- **REQ-5:** A nested tool invocation MUST NOT begin after owner validation fails.
- **REQ-6:** Interruption MUST enumerate and terminate only cells owned by the interrupted turn.
- **REQ-7:** Normal same-turn notifications, nested tools, yielded-cell waits, and cell reuse rules
  MUST remain intact.

### Non-Functional Requirements

| Category | Requirement | Target | Measurement | Evidence / Verification |
|---|---|---|---|---|
| Correctness | No stale callback becomes model-visible | Zero stale injections | Deterministic race tests | Core Code Mode tests and integration scenario |
| Concurrency | Ownership checks remain valid across worker drop | No unchecked spawned path | Code review plus interleaving tests | `delegate.rs` tests |
| Compatibility | No persisted format or external protocol changes | Zero schema changes | Diff inspection | No rollout/API changes |
| Resource use | Stale work is cancelled or rejected promptly | Bounded orphan work | Cancellation and shutdown tests | Worker lifecycle tests |

## 3. Key Invariants and Assumptions Audit

| Invariant | Enforcement Mechanism | Verification Method | Responsible Component |
|---|---|---|---|
| A cell has one owner for its lifetime | `CellOwner` stores immutable turn and generation | Registration/reuse tests | Dispatch broker |
| A worker generation is unique within a broker | Monotonic `AtomicU64` allocation | Unit test with successive workers | Dispatch broker |
| Closed cells cannot dispatch | Close invalidates the gate before any host call | Close-during-callback test | Dispatch broker |
| Model-visible injection targets the same active turn | `inject_if_turn_running(owner.turn_id, ...)` under the session active-turn lock | Stale-turn integration test | Session + broker |
| Interrupt affects only the selected turn | Owner tuple filtering | Two-turn interruption test | Code Mode service |

**Assumptions Register**

| Assumption | Evidence / Validation | Risk if False | Consequence When Violated |
|---|---|---|---|
| `Session::inject_if_turn_running` checks and queues atomically | [`inject.rs:47-78`](../codex-rs/core/src/session/inject.rs#L47-L78) | A callback could pass validation and target a replacement turn | Revisit the session API before implementation |
| Worker `Drop` can occur while spawned invocation tasks still run | [`delegate.rs:187-223`](../codex-rs/core/src/tools/code_mode/delegate.rs#L187-L223) | The race remains reachable | Final task-side validation is mandatory |
| Cell IDs can be delayed across turn boundaries | CF-007 evidence in [`BUGS.md`](../BUGS.md#cf-007-code-mode-callbacks-route-to-stale-active-turns-via-shared-broker) | Stale routing may be dismissed incorrectly | Preserve generation checks even if IDs are usually unique |

## 4. Responsibilities and Scope Boundaries

### Responsibilities

- `CodeModeDispatchBroker` owns cell capabilities, worker generations, admission, and final
  dispatch validation.
- `CodeModeDispatchWorker` owns the worker lifetime and invalidates its generation on drop.
- `CoreTurnHost` performs model-visible notification injection only after broker validation.
- `CodeModeService` requests interruption for cells owned by a specified turn.
- `Session` remains the authority for whether a turn is active and for atomic input enqueueing.

### Responsibilities Outside This Scope

- Code Mode runtime scheduling and cell execution semantics.
- Provider billing, output truncation, and ordinary tool authorization.
- Persisted rollout schema or app-server protocol changes.
- Changing the product behavior of automatic continuation turns.

## 5. Security and Threat Model

This is a lifecycle-integrity boundary rather than an authentication boundary. No new external
principal or privilege is introduced.

```text
Code Mode runtime
      | callback(cell_id)
      v
[Dispatch broker: cell owner + generation] --reject stale--> dropped callback
      | validated dispatch
      v
[Per-turn worker] --final owner check--> [Session active-turn lock]
                                             |
                                             v
                                      model-visible input
```

**Attack Paths / Failure Paths**

1. A delayed callback names a cell from turn 1 after turn 2 starts. The broker rejects it because
   the owner generation or active turn does not match; no input is queued.
2. A callback is queued just before worker drop. The spawned task revalidates the gate and worker
   generation before submitting or injecting; it returns an owner-ended error.
3. A cell closes while a callback is waiting. Gate invalidation wakes the waiter, which rejects the
   callback and does not remove a replacement cell's gate.

| Risk | Attack Path | Likelihood | Impact | Risk Treatment | Residual Risk | Owner |
|---|---|---:|---:|---|---|---|
| Stale model context | Delayed runtime callback | Medium | Medium token burn and context pollution | Generation-scoped capability plus final check | Low | Core Code Mode |
| Orphaned nested tool | Worker drops after spawn | Medium | Medium unnecessary work | Task-side cancellation and owner validation | Low | Dispatch broker |
| Wrong interruption | Cell ownership lookup races turn change | Low | Unrelated work terminated | Match complete owner tuple | Low | Code Mode service |

**Risk Acceptance Criteria:** The change is acceptable only when deterministic tests demonstrate
that all three paths produce no model-visible stale item, and same-turn callback tests continue to
pass.

## 6. Privacy and Data Minimization

No personal or sensitive data is processed by the ownership metadata. The broker stores only the
existing session weak reference, turn identifier, cell identifier, and an in-process generation
number. Callback text remains subject to the existing tool/session retention rules. Generation
values MUST NOT be serialized into rollouts, logs containing user content, or external protocol
payloads.

## 7. Key Concepts

| Term | Description |
|---|---|
| Cell | A Code Mode runtime execution identified by `CellId`. |
| Owner capability | Immutable session, turn, and worker-generation identity associated with a cell. |
| Worker generation | Monotonic broker-local identity for one per-turn dispatch worker instance. |
| Admission check | Validation before a callback enters a worker queue. |
| Final check | Validation immediately before nested-tool submission or session injection. |

## 8. Architecture

### 8.1 Key Components

- `CodeModeDispatchBroker`: maps cells to owners and turns to workers.
- `CellDispatchGate`: stores the owner capability and originating item metadata.
- `TurnDispatchWorker`: stores generation, sender, and worker lifetime.
- `CoreTurnHost`: delegates nested tools and notifications to the session.
- `Session`: atomically determines active-turn identity and accepts matching input.

### 8.2 Trust Boundaries and Data Flows

The Code Mode runtime is an asynchronous producer. The broker is the least-privilege boundary: it
may route a callback only to the capability recorded when the cell became dispatchable. The session
active-turn lock is the final authority. Callback text crosses into model-visible history only
after both boundaries succeed.

### 8.3 Cryptographic and Protocol Behavior

Not applicable. The generation is an in-process concurrency token, not a security credential.

## 9. Type Safety Model and API Contracts

| Type or API | Purpose | Safety Guarantees | Invariant Enforcement | Misuse Prevention |
|---|---|---|---|---|
| `CellOwner` | Immutable callback owner | Carries turn and generation together | Constructed at registration | Callers cannot supply a bare destination worker |
| `CellDispatchGate` | Cell lifecycle state | Gate invalidation is observable to waiters | Close clears owner before removal | Closed cells cannot pass final validation |
| `TurnDispatchWorker` | Per-turn worker | Generation identifies exact worker instance | Monotonic allocation and generation-checked removal | Old worker cannot remove replacement |
| `Session::inject_if_turn_running` | Atomic model-input admission | Checks exact turn while holding active-turn state | Existing session lock | A replacement turn cannot receive the item |

The implementation SHOULD use a private owner-validation result (for example, an enum distinguishing
`Closed`, `WorkerGone`, and `TurnEnded`) rather than boolean parameters or ambiguous optional values.
No unsafe Rust is required.

## 10. Control Flow

1. `start_turn_worker` allocates a generation and registers the worker.
2. `mark_cell_ready_for_dispatch` records the cell owner with that turn's generation.
3. Runtime callback waits for readiness, then snapshots the owner capability.
4. Broker verifies the owner still maps to the same cell and worker generation and that the session
   reports the owner turn active.
5. The worker receives a message carrying the owner capability, not merely `cell_id`.
6. Immediately before `submit_tool` or `notify`, the task repeats the gate/generation/active-turn
   check. Failure returns a bounded owner-ended error.
7. Notification uses `inject_if_turn_running` with the owner turn ID.
8. Worker drop invalidates its generation; cell close invalidates the cell gate and wakes waiters.

## 11. Data Model

`CellOwner` contains `turn_id: String`, `generation: u64`, and the existing weak session reference.
`DispatchMessage::InvokeTool` and `DispatchMessage::Notify` carry the owner capability (or an
equivalent private validation token). `CellDispatchGate` retains the originating item ID exactly
as it does today. No field is persisted or exposed outside the broker.

## 12. Error Handling

Stale, closed, unavailable, and cancelled callbacks return existing string-based Code Mode errors
with stable, non-sensitive descriptions. They MUST NOT be converted into successful empty tool
outputs, queued for a later turn, or logged with callback text. Channel closure remains a dispatcher
unavailable error. The existing notification error path in [`delegate.rs:467-487`](../codex-rs/core/src/tools/code_mode/delegate.rs#L467-L487)
remains the final diagnostic boundary.

## 13. Concurrency, Lifetimes, Robustness, and Resource Management

| Concern | Rule / Guarantee |
|---|---|
| Thread safety | Shared maps remain protected by their existing mutexes; owner values are cloneable and immutable. |
| Lock ordering | Do not hold the broker mutex across session awaits, channel sends, or host futures. |
| Cancellation | Cancellation wins over waiting and queued stale work; task-side validation still runs for races. |
| Shutdown | Worker drop invalidates its generation and closes its receiver; no callback is rerouted. |
| Backpressure | Preserve the existing channel behavior; stale messages are rejected before model-visible work. |
| Resource exhaustion | A delayed callback cannot create a new worker or retain an unbounded history item. |

| Trigger | Component | Expected Behavior | Prohibited Behavior | Impact | Mitigation | Verification |
|---|---|---|---|---|---|---|
| Turn 1 ends before callback | Broker | Reject callback | Inject into turn 2 | Context pollution | Active-turn + generation check | Delayed callback test |
| Worker drops after queue send | Worker task | Reject before host call | Submit nested tool after drop | Orphaned inference | Final task-side check | Drop interleaving test |
| Cell closes while waiting | Gate | Wake and reject waiter | Bind to replacement cell | Wrong-cell routing | Invalidate gate and generation | Close/wait test |
| Turn interrupt | Service | Terminate only matching cells | Terminate cells from another turn | Unrelated work loss | Complete owner filtering | Two-turn interruption test |

## 14. Configuration

Not applicable. Ownership enforcement is unconditional and has no configuration switch.

## 15. Dependencies and Supply-Chain Risks

No dependency changes are required. The design uses existing `Arc`, `Weak`, mutex, atomics,
channels, and cancellation primitives. Cargo manifests and lockfiles remain unchanged.

## 16. Common Patterns

- Pass the owner capability through every asynchronous boundary.
- Perform the final validation in the task that is about to cause model-visible work.
- Keep session locks scoped to the atomic check-and-enqueue operation.
- Use generation-checked removal so an old worker cannot delete a replacement worker entry.

## 17. Common Issues

| Symptom | Cause | Investigation | Fix |
|---|---|---|---|
| Stale text appears in a later turn | Final task lacks owner validation | Trace callback, worker, and session turn IDs | Carry and validate owner generation |
| Valid same-turn callback is dropped | Gate closes too early | Inspect cell lifecycle around initial response and wait | Preserve gate until terminal cell close |
| Old worker removes new worker | Removal matches only turn ID | Inspect worker drop map removal | Match turn ID and generation |
| Test is timing-dependent | Race has no synchronization points | Use barriers/oneshots around worker drop and callback | Make interleaving deterministic |

## 18. Verification Coverage

| Verification Method | Location | Purpose | Requirement and Invariant Coverage | Evidence |
|---|---|---|---|---|
| Broker unit tests | `codex-rs/core/src/tools/code_mode/delegate_tests.rs` or existing dedicated test module | Registration, generation, close, and stale rejection | REQ-1–REQ-3 | Required implementation evidence |
| Deterministic callback race test | Core Code Mode tests | Turn 1 callback after turn 2 starts | REQ-2–REQ-5 | Required implementation evidence |
| Spawn/drop interleaving test | Core Code Mode tests | Queued task after worker drop | REQ-3, REQ-5 | Required implementation evidence |
| Interrupt isolation test | Core Code Mode/service tests | Only owner turn cells terminate | REQ-6 | Required implementation evidence |
| Same-turn regression test | Existing Code Mode integration coverage | Preserve notifications, nested tools, yielded waits | REQ-7 | Required implementation evidence |
| Formatting and lint | `cd codex-rs` then `just fmt` and `just fix -p codex-core` | Repository conformance | All Rust changes | Required delivery evidence |
| Targeted tests | `cd codex-rs` then `just test -p codex-core` | Behavioral verification | All functional requirements | Required delivery evidence |

The repository currently provides no dedicated Code Mode broker test file in the inspected source
tree; this is an explicit evidence gap to close by adding a dedicated sibling test module or a
focused integration test without placing test helpers in production APIs.

## 19. Debugging and Observability

Structured diagnostics MAY record cell ID, turn ID, worker generation, and rejection reason. Logs
MUST omit callback text and model payloads. A stale rejection should be distinguishable from normal
cancellation and channel shutdown so operators can confirm the protection is active.

## 20. Performance, Scalability, and Robustness Analysis

The workload is one broker lookup and one bounded ownership validation per callback, plus the
existing session active-turn check. The generation comparison is constant time and adds no model
request or persistence work. The design MUST preserve the existing worker-per-turn and cell-map
complexity. Robustness acceptance requires zero stale injections under repeated deterministic
interleavings and no measurable unbounded map growth after cell close.

## 21. Compatibility, Deployment, and Migration Boundaries

**Approved behavior change — stale callback rejection:** callbacks from ended turns are discarded
with an error instead of being allowed to reach a later active turn. This is authorized by CF-007's
token-burning defect definition and affects only invalid lifecycle behavior.

Same-turn Code Mode behavior, persisted rollout data, app-server APIs, and runtime protocols remain
compatible. No migration or deployment ordering is required. Rollback is a source rollback; no
stored data needs conversion. The change is accepted only after targeted core tests pass and the
diff confirms no unrelated crate or protocol modifications.

## 22. Alternatives Considered

| Alternative | Advantages | Disadvantages | Decision Rationale |
|---|---|---|---|
| Check only `is_turn_running` before queueing | Small patch | Race remains between check, queue, and task execution | Rejected; does not establish the required invariant |
| Cancel all Code Mode cells whenever any turn ends | Simple lifecycle | Breaks yielded cells and unrelated active work | Rejected; violates per-turn ownership |
| Create one broker per turn | Strong isolation | Requires larger runtime plumbing and complicates yielded-cell lookup | Rejected; generation-scoped ownership fits existing broker |
| Add only cancellation tokens | Handles cooperative cancellation | Cancellation can race and does not prove target turn identity | Rejected as sole control; retain cancellation as a supplement |

## 23. Related Documentation and Source References

| Reference | Relevance |
|---|---|
| [`BUGS.md` CF-007](../BUGS.md#cf-007-code-mode-callbacks-route-to-stale-active-turns-via-shared-broker) | Defect trigger, impact, and required verification |
| [`delegate.rs`](../codex-rs/core/src/tools/code_mode/delegate.rs) | Broker, worker, gate, callback, and host implementation |
| [`mod.rs`](../codex-rs/core/src/tools/code_mode/mod.rs) | Service registration, interruption, and worker lifecycle |
| [`execute_handler.rs`](../codex-rs/core/src/tools/code_mode/execute_handler.rs) | Cell readiness and terminal-close lifecycle |
| [`inject.rs`](../codex-rs/core/src/session/inject.rs) | Atomic identified-turn injection contract |
| [`core/README.md`](../codex-rs/core/README.md) | Core crate platform and test context |
