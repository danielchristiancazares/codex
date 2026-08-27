The following is a message generated after compaction from a previous session. Continue naturally on the active task.

The complete previous conversation is available in the [previous rollout](ROLLOUT_PATH). Read it whenever exact wording, chronology, or omitted context could affect the work. The user messages quoted below retain their original order and meaning; later messages govern where instructions changed.

Current system, developer, and repository instructions still apply. Read the relevant `AGENTS.md` files before editing. Verify workspace state before relying on these notes.

Replace every placeholder, remove inapplicable sections, and keep the completed handoff within 8,000 UTF-8 bytes. Use the rollout for lower-priority detail.

# Active task

## Goal

> [The user's active goal, copied verbatim where practical.]

## What completion means

- [Observable completion criterion.]
- [Required artifact, behavior, or answer.]
- [Required tests or validation.]
- [Commit, push, cleanup, or external-state requirement, when applicable.]

## Current status

**Status:** In progress

[Two or three sentences describing where the task currently stands and why it is still active.]

# User messages that govern the work

These are the user messages whose wording still affects the task. They appear chronologically.

1. > [Exact user message establishing the task.]

2. > [Exact user clarification or constraint.]

3. > [Exact latest instruction.]

The rollout contains the complete user-message sequence. Use its exact wording whenever a detail here appears incomplete or ambiguous.

# Preserved inputs and large pasted content

## User message [number]

The visible user text was:

> [Exact visible user text.]

The same message included:

> [Exact platform-generated pasted-content or attachment marker.]

The pasted content was preserved before the previous session ended:

- **Durable copy:** `[path/to/handoff/artifacts/file]`
- **Original temporary path:** `[original temporary path]`
- **SHA-256:** `[hash]`
- **Size:** `[bytes or approximate word count]`
- **Purpose in the task:** [Source material, specification, logs, code, or currently unclear.]
- **Relevant sections already examined:** [Line ranges or headings.]
- **Sections still requiring review:** [Line ranges, headings, or "entire document".]

Read the preserved file when its contents are needed. Treat the surrounding user wording as the authority for how the pasted material should be used.

# Work completed

- [Completed action, including paths or identifiers.]
- [Completed action and resulting behavior.]
- [Artifact created or updated.]
- [External action completed, if any.]

# Current workspace state

- **Working directory:** `[absolute path]`
- **Branch:** `[branch name]`
- **HEAD:** `[commit hash]`
- **Upstream/divergence:** `[state]`

## Task-owned changes

- `[path]` - [What changed and why.]
- `[path]` - [What changed and why.]

## Pre-existing or unrelated changes

- `[path]` - [Classification and why it must be preserved.]

## Untracked files

- `[path]` - [Purpose and ownership.]

## External state

[Relevant pull request, issue, process, deployment, service, remote branch, or "none".]

# Important decisions already made

- **[Decision]:** [Reason, user instruction, and supporting evidence.]
- **[Decision]:** [Reason and consequences for the remaining work.]

# Commands and validation

## Passed

- `[exact command]`
  - Working directory: `[directory]`
  - Result: [Meaningful result.]

## Failed

- `[exact command]`
  - Working directory: `[directory]`
  - Result: [Exact failure or concise error.]
  - Meaning: [What was learned.]

## Still required

- `[exact command or validation step]`
  - Reason: [Why it remains necessary.]
  - Expected duration or prerequisite: [When relevant.]

# Approaches already attempted

- **[Approach]:** [Observed result and lesson.]
- **[Approach]:** [Why it was rejected or what would need to change before retrying.]

# Remaining work

1. [Next coherent step.]
2. [Following implementation or investigation step.]
3. [Required validation.]
4. [Delivery, commit, push, cleanup, or final report.]

# Next action

Start by:

> [One precise action, including the exact file, command, or question.]

Then continue through the remaining work without repeating completed steps unless current verification shows that the state has changed.

# Open questions and uncertainties

- [Fact that still needs verification.]
- [Decision that belongs to the user.]
- [Potential risk or ambiguity.]

# Sensitive artifacts and cleanup

- `[path or identifier]` - [Why it exists, handling requirement, and eventual cleanup.]
- [State whether anything was transmitted externally, when relevant.]

# Completion checkpoint

The task is complete when:

- [Criterion.]
- [Criterion.]
- [Validation.]
- [Requested delivery action.]

At handoff time, these criteria were still outstanding:

- [Outstanding item.]
- [Outstanding item.]
