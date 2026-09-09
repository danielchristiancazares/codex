# Working on the personal fork

The studio has one maintainer and one AI collaborator. The maintainer sets the
goal and decides product behavior, commits, publication, and releases. The AI
inspects, implements, validates in proportion to risk, reviews the diff, and
finishes the authorized work. Existing authorization carries across turns.
Use a pull request when its review or CI record is useful; direct work on the
product branch is the normal path.

## Branch ownership

| Name                    | Purpose                                                  |
| ----------------------- | -------------------------------------------------------- |
| `upstream/main`         | OpenAI development reference                             |
| `refs/tags/rust-vX.Y.Z` | Selected stable OpenAI release used as the rebase target |
| `main`                  | Maintained product, with a small series of fork commits  |
| `origin/main`           | Published product history                                |
| `backup/*`              | Recovery refs created before substantial history changes |

Local `main` tracks `origin/main`. Ordinary pushes publish to `origin`, and
`git pull` only fast-forwards from the published fork. Integrate OpenAI changes
only when adopting a selected, published stable Codex release tag of the form
`rust-vX.Y.Z`. Alpha, beta, and release-candidate tags are excluded from routine
syncs. Fetching development commits or discovering a new tag does not trigger a
rebase.

Select the tag from OpenAI's [published releases](https://github.com/openai/codex/releases).
Confirm it is a completed stable release, then use the explicit `--onto` procedure
below. Release branches can have different ancestry from `upstream/main`.

The original fork tip and migration boundaries are recorded in
[upstream-main-migration.md](upstream-main-migration.md). Keep `main` as the
personal GitHub repository's default branch so new work and repository browsing
start from the maintained product.

## Installing this checkout

`just install` builds a release package for the current host with
`scripts/build_codex_package.py`, then updates the native payload of the existing
global npm `@openai/codex` installation. It includes the code-mode host, bundled
`rg`, and platform resources. The npm launcher remains in place.

The installer verifies the package before replacing the payload and restores the
previous payload if the replacement or launcher check fails. The global npm
package directory must be writable. On Windows, close processes using the
installed executables before updating. V8 versions and artifact selection follow
the package builder; explicit V8 archive/binding overrides remain available.

Use `just assemble-codex-package` when only a package artifact is needed.

## Daily work

1. Inspect `git status --short --branch`, the diff, and relevant instructions.
   Before changing history, also inspect remotes, tracking, linked worktrees,
   stashes, and ahead/behind counts.
2. Keep one active writer in a worktree. Use a separate worktree for an independent
   experiment; partition files explicitly if concurrent editing is necessary.
   Finish edits before starting final validation.
3. Implement one coherent behavior at a time. Keep generated schemas, snapshots,
   and lock changes with the behavior that requires them. Avoid unrelated
   formatting or refactoring in the same patch.
4. Run the smallest useful validation from the repository root, for example
   `just test -p codex-tui <filter>`. Broaden for shared behavior or unresolved
   failures. Preserve required integration and snapshot coverage. Check required
   tools before long runs. Finish large Rust changes with scoped `just fix` and
   `just fmt`, without rerunning tests afterward.
5. Before an authorized commit, classify every staged, unstaged, and untracked
   path. Stage explicit paths and review the staged patch, stat, and
   `git diff --cached --check`. Include only task-owned changes. Commit and push
   when requested, then verify the resulting refs.

Keep durable decisions in focused `docs/` files. The migration notes describe
the current integration boundaries; `PERFORMANCE_LOG.md` owns performance
measurements and rejected experiments. Record unfinished work and validation
limits when handing a task back. Temporary logs and build artifacts stay out of
commits. Validate performance changes with a warmed release baseline, one
candidate at a time, and a final measurement of the retained state.

## Repository-local Git defaults

These settings are applied to this clone. Linked worktrees share them; a new
clone needs them applied again. Use Git 2.38 or newer. Run from the repository:

```sh
git config --local remote.pushDefault origin
git config --local branch.main.pushRemote origin
git config --local push.default current
git config --local push.autoSetupRemote true
git config --local pull.rebase false
git config --local pull.ff only
git config --local fetch.prune true
git config --local rerere.enabled true
git config --local rerere.autoupdate false
git config --local merge.conflictStyle zdiff3
git config --local rebase.autoStash false
git config --local rebase.updateRefs false
```

Pushes default to the same branch name on `origin`. Pulls only fast-forward,
leaving upstream rebases explicit. Fetching prunes stale remote-tracking branches.
`rerere` remembers conflict resolutions and reapplies matching ones, while leaving
them for review and staging. `zdiff3` includes the common-base text in conflicts.
Rebases require a clean worktree and leave other branch refs, including recovery
branches, in place. There are no automatic pushes or history rewrites.

Git stores this configuration and recorded resolutions locally; neither is
published with commits. The setup commands above are the reproducible record.

## Updating from OpenAI

The recorded upstream base is currently:

| Field  | Value                                         |
| ------ | --------------------------------------------- |
| Source | Initial migration snapshot of `upstream/main` |
| Commit | `b4373e53ab79df7baadc6805dea54a060b820307`    |

This initial migration predates the release-tag policy. For the first tagged
sync, wait for a stable release whose history includes this snapshot. After each
successful sync, update this table with the selected release tag and its resolved
commit SHA. That commit is `OLD_UPSTREAM_BASE` for the next sync; do not infer it
from the moving `upstream/main` ref.

Start on `main` with committed work and a clean index/worktree. If unrelated
changes are present, preserve them explicitly before continuing.
Replace `RELEASE_TAG` below with the selected tag, and `OLD_UPSTREAM_BASE` with
the commit recorded above.

```sh
git fetch origin
git fetch upstream tag RELEASE_TAG
git log --oneline main..origin/main
```

If the log shows remote commits absent from the local branch, inspect and
reconcile them before the rebase. Verify the recorded base is an ancestor of
`main`, review the fork commits to replay, and record the release tag's commit,
the current remote tip, and the backup ref in task notes:

```sh
git merge-base --is-ancestor OLD_UPSTREAM_BASE main
git log --oneline OLD_UPSTREAM_BASE..main
git rev-parse 'refs/tags/RELEASE_TAG^{commit}'
git rev-parse origin/main
git branch "backup/main-$(date +%Y%m%d-%H%M%S)" main
```

Replay only the fork commits onto the selected release:

```sh
git rebase --onto refs/tags/RELEASE_TAG OLD_UPSTREAM_BASE main
```

The explicit old base is required: a plain rebase onto a release tag can also
replay upstream commits from a different development or release branch.

Review every conflict against both behaviors, including reused resolutions.
`git rerere diff` shows the recorded resolution changes. Stage reviewed paths
explicitly, then use `git rebase --continue`. `git rebase --abort` returns an
in-progress rebase to its starting state.

Afterward, compare the old and new commit series with:

```sh
git range-diff OLD_UPSTREAM_BASE..BACKUP_REF refs/tags/RELEASE_TAG..main
```

Replace the uppercase placeholders with the recorded values. Drop fork patches once
upstream supplies equivalent behavior, and check that surviving integration
points still control the relevant code paths. Validate affected crates and UI
snapshots before the final lint/format pass.
Update and commit the recorded upstream base after the sync passes validation.

When publication of the rewritten branch is authorized, pin the lease to the
remote tip recorded before the rebase:

```sh
git push --force-with-lease=refs/heads/main:RECORDED_REMOTE_TIP origin main
```

The explicit lease still checks that original tip if an editor fetches in the
background. A rejected lease requires inspecting the new remote work. Verify the
published tip matches the intended commit before reporting the push complete.
