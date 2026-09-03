---
name: backport
description: Backport an existing committed change or GitHub pull request onto an older or divergent target branch while preserving source intent, fork-specific behavior, and commit provenance. Use whenever the task is to carry an already-implemented change across branches or releases in this repository; representative requests include backporting, cherry-picking, transplanting, or porting a commit, commit range, or pull request.
---

# Backport

## Objective

Carry an existing change onto its target branch as the smallest complete semantic patch. Preserve
the destination branch's established behavior, this fork's stronger invariants, and a reviewable
link to every source commit.

Treat `origin` as the maintained personal fork and `upstream` as OpenAI's source repository. Keep
pushes, pull requests, releases, and history rewrites within the user's explicit request.

## 1. Establish a safe baseline and refresh upstream

Read the root `AGENTS.md` and every nearer `AGENTS.md` governing touched paths before applying the
change or choosing validation.

Inspect the repository before fetching, switching branches, or applying commits:

```text
git status --short --branch
git branch -vv
git remote -v
git worktree list --porcelain
git stash list
git rev-parse HEAD
```

Record the current branch and HEAD as the worktree baseline. Classify every reported staged,
unstaged, untracked, and nested-repository change as task-owned or pre-existing. Preserve each
pre-existing path, its index state, and ignored workspace data. Use an isolated worktree when it
cleanly protects concurrent work. Agree on a plan with the user when the backport overlaps work
that cannot be isolated safely.

After recording the baseline, refresh `upstream/main` on every invocation, including invocations
with an explicit source or alternate target:

```text
git fetch upstream main:refs/remotes/upstream/main
git rev-parse upstream/main
```

Continue the workflow only after the fetch succeeds. A failed fetch leaves default-source
freshness unestablished; report the exact network, authentication, remote, or ref blocker and
resume after a successful fetch.

## 2. Resolve the source and destination

Establish:

- the source PR, commit, ref, ordered commit list, or range;
- the source repository and remote;
- the target branch and worktree;
- the intended destination commit boundaries; and
- whether the task includes a push, PR, or release action.

Treat explicitly named sources and targets as authoritative. Resolve a bare number as an
`openai/codex` pull request. Explicit repository context takes precedence.

Always use the exact freshly fetched `upstream/main` tip commit as the source when the user leaves
the PR number, URL, commit, range, or source ref unspecified. Record its SHA and treat that single
commit as the complete default source delta. Proceed directly from this deterministic source.

When the user omits the target, use the current branch and HEAD recorded in step 1 as the
destination baseline. Keep a named current branch as the destination branch. For a detached HEAD,
materialize a descriptive `backport/<source>` destination branch rooted at the recorded baseline.
Use an isolated worktree or destination branch when workspace safety requires separation from
pre-existing changes.

Fetch any additional source remote and ref without moving the destination branch. Resolve the
source objects and destination baseline, then materialize any required local destination branch.
Record its HEAD as the pre-backport target SHA and confirm it is at the intended starting point.
Create a dated backup ref before a broad or history-changing integration.

## 3. Reconstruct the source delta

For a PR, inspect its repository, state, base, commits, merge commit, and changed files:

```text
gh pr view <number-or-url> --repo <owner/repo> --json number,url,state,baseRefName,headRefName,commits,mergeCommit,files
```

Fetch the selected objects into the local object database, then inspect their content and topology:

```text
git show --stat --summary <source-sha>
git show --find-renames <source-sha>
git rev-list --parents -n 1 <source-sha>
git merge-base <target> <source-sha>
```

Choose the source representation that reproduces the complete integrated change, or the requested
head state for an open PR. Representative merge mappings include:

- a squash-merged PR represented by its single merge commit;
- a true merge commit represented by that commit with its verified target-branch parent as the
  mainline; and
- a rebased PR represented by its PR commits in oldest-to-newest order.

Verify the selected SHA or sequence against the PR's changed-file diff. For a merge commit, compare
the mainline parent directly with the merge result. Establish whether the target already contains
an equivalent patch, a partial implementation, or prerequisite work. Search destination history
and code because equivalent changes can have different hashes.

Capture the source change's behavioral intent, tests, generated artifacts, dependency updates, and
compatibility effects. Trace every affected producer and consumer, including protocol, event,
request-pipeline, authentication, persistence, and public-API paths according to `AGENTS.md`.

## 4. Apply in logical order

Prefer Git's native cherry-pick machinery and preserve source commit boundaries. Apply multiple
commits oldest first:

```text
git cherry-pick -x <source-sha>
git cherry-pick -x <oldest-sha> <next-sha> <newest-sha>
```

Use `-x` to record source provenance. When target-specific adaptation requires a manually created
commit, include every represented source SHA in the commit body and preserve source authorship for
a one-to-one mapping. Stage task-owned paths explicitly, run `git diff --cached --check`, and review
the staged stat and patch before committing.

For a merge commit, verify its parent ordering before selecting the mainline parent:

```text
git cherry-pick -x -m <mainline-parent-number> <merge-sha>
```

Resolve conflicts as semantic integration work:

1. Inspect the unmerged paths and index stages.
2. Read the full source change, destination implementation, nearby tests, and relevant history.
3. Preserve destination-specific APIs and stronger state representations while carrying over the
   source behavior.
4. Edit each resolution deliberately, stage explicit paths, and review the staged result.
5. Continue only after every source-intent item is represented.

```text
git status --short
git diff --name-only --diff-filter=U
git ls-files -u
git diff --cached --check
git diff --cached
git cherry-pick --continue
```

Use file-level `ours` or `theirs` only when the chosen whole file expresses the intended result.
When a cherry-pick becomes empty, prove that the destination already contains the full source
intent before skipping it, and retain that evidence for the report. When a missing prerequisite
expands the requested scope, inspect it and obtain the user's decision before adding it.

If source or target selection proves incorrect, use `git cherry-pick --abort`, verify the recorded
baseline is restored, and restart with the corrected selection. Reserve force pushes and history
rewrites for explicit authorization.

## 5. Validate the result

Review the destination range as its own change and against the source:

```text
git diff --check <pre-backport-sha>..HEAD
git diff --stat <pre-backport-sha>..HEAD
git diff <pre-backport-sha>..HEAD
git log --format=fuller --decorate <pre-backport-sha>..HEAD
```

Use `git range-diff` for a comparable multi-commit series. Confirm that source behavior and tests
are present, conflict resolutions preserve fork behavior, generated files and lockfiles align,
commit messages retain provenance, and the range contains only task-owned changes.

Run the narrowest relevant checks first, followed by every broader check required by the governing
`AGENTS.md`. Obtain approval before a required long full-suite run when repository instructions call
for it. Run final formatting in the prescribed order. Recheck worktree state after validation.

When a push is requested, use a regular push and verify the destination remote ref contains the new
commit. Use force only under explicit history-rewrite authorization.

## 6. Report

Report the source and remote, target branch, pre-backport and final SHAs, clean picks, skipped
equivalents, semantic adaptations, validation outcomes, and commit/push state. State any remaining
risk, unverified platform, or pending user decision.

Request a user decision whenever progress would choose product behavior or risk existing work.
Representative cases include unresolved source or target ambiguity, overlapping worktree changes,
conflicts with multiple valid behaviors, and prerequisite commits that materially expand scope.
