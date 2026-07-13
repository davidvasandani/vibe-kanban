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
- **There is a dormant second selector.** `useRepoBranchSelection.ts` +
  `RepoBranchSelector.tsx` implement a `userOverride -> initialBranch ->
  default_target_branch -> is_current -> first` chain but have **no importers**.
  They are NOT wired into the create-mode screen and their fallback prefers the
  current branch, not `origin/main`. If you touch branch defaults, know this
  divergent logic exists; reconciling or deleting it is a separate task.

## Contributed by

- `vk/c59f-default-to-origi` — default the create-mode repo picker to
  `origin/main`; introduced `resolveDefaultBranch` and documented this seam.
