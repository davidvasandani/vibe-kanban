# Research: Repository-scoped Git panel PR links

## Existing behavior

`GitPanelContainer` first looks for a repository-specific status entry and PR
merge. When `branchStatus` is unavailable, it falls back to `summaryPr` derived
from the selected workspace and repeats that fallback inside every `repos.map`
iteration. Because the summary has no `repo_id`, all rows receive the same PR.

The local branch-status response already provides the required association:
`RepoBranchStatus.repo_id`, with repository-owned `merges: Merge[]`. `PrMerge`
also includes `repo_id`, but it is reached through the already-scoped status.

## Decisions

### Remove rather than constrain the aggregate fallback

Applying the workspace summary only when `repos.length === 1` would repair the
reported multi-repo symptom, but would still guess identity and create divergent
single-/multi-repo rules. The summary cannot prove which repository owns its PR,
so it is not used for row state.

### Extract a pure transformation

The defect is a deterministic association policy, not a rendering detail. A
pure helper gives direct fixtures for sibling isolation and loading behavior and
avoids extensive provider mocking.

### Preserve current PR precedence

Within one repository, the existing code prefers an open PR to a merged PR.
That behavior is retained to keep the change scoped to association.

### Do not infer repository from URL

Parsing GitHub owner/repository strings and matching them to registered remotes
would duplicate remote-resolution rules and still fail across providers or
renamed remotes. Explicit backend identity is the appropriate future contract
for manual remote PR discovery.

## Dependencies

None added.
