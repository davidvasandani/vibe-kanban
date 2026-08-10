# Create-mode repo branch defaulting

How the create-issue screen ("Which repositories would you like to work on?")
chooses a repository's target branch, and the seam to change that default.

## Where the screen lives

- `CreateChatBoxContainer.tsx` renders the `repoStep` heading and, while in the
  repo-picker step, `CreateModeRepoPickerBar.tsx` — the repo rows + branch
  button + Recent/Browse/Create actions. This is the create-**mode** chat flow,
  distinct from `KanbanIssuePanel.tsx` (the local-kanban issue create/edit
  panel — see [[kanban-issue-panel-sections]]). Don't confuse the two "create"
  surfaces.
- Selection state lives in the `create-mode` feature
  (`useCreateModeState.ts`): each repo carries a `targetBranch: string | null`;
  the derived `targetBranches` map feeds the row label and the submit guard
  `hasSelectedBranchesForAllRepos` (a repo with a null branch blocks submit).

## The branch-defaulting seam

All three add paths (Recent / Browse / Create) funnel through
`CreateModeRepoPickerBar.addRepoWithBranchSelection(repo)`. That one function is
the single place a newly-added repo's default branch is decided. The
"Change branch" button uses a *separate* path (`handleChangeBranch` →
`pickBranchForRepo`, the `SelectionDialog` modal), so changing the default and
changing an individual branch are independent — edit `addRepoWithBranchSelection`
for the default, leave `pickBranchForRepo` for the override.

Historically `addRepoWithBranchSelection` *forced* the modal (`pickBranchForRepo`)
and refused to add the repo if the user cancelled — no default at all. Task
c59f replaced that with an automatic default via a pure helper
`resolveDefaultBranch(branches, preferredBranch?)`
(`src/shared/lib/defaultBranch.ts`):

```
configured default_target_branch  (explicit user choice wins)
  -> origin/main                  (the built-in default)
  -> origin/master                (main/master naming fallback)
  -> the is_current branch
  -> the first branch
  -> null                         (only when the branch list is empty)
```

An empty list is handled by setting a picker error and *not* adding the repo,
which preserves the submit guard's "every repo has a branch" invariant.

## Non-obvious gotchas

- **Branch names carry the remote prefix.** `repoApi.getBranches` →
  `GET /api/repos/{id}/branches` → `git::get_all_branches` returns `GitBranch`
  whose `name` is `origin/main` for remote-tracking branches and `main` for
  local ones (`is_remote` distinguishes them). So a default of "origin/main"
  must match the literal string `origin/main`, not `main`. Match by exact
  `name`; there is no separate "default branch" field from git.
- **`get_all_branches` sorts current-branch-first**, then by most recent commit
  — so `branches[0]` is the *current* checkout, not `origin/main`. Don't rely on
  list order for a mainline default; select by name. Changing this backend sort
  would still not set the frontend default (the picker selects by name), so
  branch-default work belongs on the frontend and keeps the fork's upstream diff
  minimal.
- **`repo.default_target_branch` is NULL at registration.** It is only set via
  repo settings (`ReposSettingsSection`), so for most repos the built-in
  `origin/main` default is what actually applies. Treat it as an optional
  override, and re-validate it still exists in the fetched branch list before
  using it (a stale configured branch should fall through, not be selected).
- **The dormant second selector shares the canonical policy.**
  `useRepoBranchSelection.ts` + `RepoBranchSelector.tsx` still have no runtime
  importers and are NOT wired into the create-mode screen. Task `vk/1476` made
  the hook preserve `userOverride -> initialBranch` precedence and then delegate
  configured/default/fallback inference to `resolveDefaultBranch`. Keep future
  selectors on this helper so a dormant or newly wired path cannot silently
  reintroduce current-checkout-first behavior.

## What the backend owes this default

The prefix is not a frontend detail. Every backend consumer of `target_branch`
must resolve it **local branch first, then remote-tracking branch** — the order
`GitService::find_branch` and `check_branch_exists` already implement, and the
one `check_branch_exists` used to accept the user's choice at
`ManagedWorkspace::add_repository`. A consumer that resolves only
`refs/heads/<name>` rejects the default, i.e. almost every workspace.

This has been shipped broken once. `SharedRepositoryStore::ensure` (added by
`19a4-git-worktrees-br`) asked for `refs/heads/{branch}` alone, and since the
shared bare store is a `clone --bare` of the checkout — `refs/heads/*` and no
`refs/remotes/*` — `origin/main` matched nothing. Every clustered workspace
created with the default branch failed with a generic "An internal error
occurred", and it looked intermittent because it is deterministic *per branch
selection*: the default always failed, a hand-picked local branch always worked.

Two rules follow for anyone adding such a consumer:

- **Do not normalise the prefix away.** `origin/main` and a local `main` can be
  different commits, and the user picked the remote one. Stripping it in the
  picker would also break the exact-`name` match against `get_all_branches`.
- **Give the consumer the refs, not a special case.** The fix was to mirror the
  checkout's `refs/remotes/*` into the store so `create_branch` and
  `git worktree add` resolve the same name the picker offered. See
  [`docs/knowledge-base/clustered-workspace-execution.md`](../docs/knowledge-base/clustered-workspace-execution.md).

## Contributed by

- `vk/c59f-default-to-origi` — default the create-mode repo picker to
  `origin/main`; introduced `resolveDefaultBranch` and documented this seam.
- `vk/b72a-internal-error-o` — recorded the backend consumer contract after the
  shared repository store shipped a second, narrower resolution rule.
- `vk/1476-protect-git-repo` — reconciled the reusable hook with the canonical
  remote-mainline policy and added hook-boundary regression coverage.
