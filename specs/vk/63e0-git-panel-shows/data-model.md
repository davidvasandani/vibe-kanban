# Data Model: Repository-scoped Git panel projection

No persistent data model changes.

## Existing inputs

### RepoWithTargetBranch

- `id`: stable local repository identifier
- `name` / `display_name`: row label
- `target_branch`: configured target

### RepoBranchStatus

- `repo_id`: identity used to join to the configured repository
- commit/remote counts and target metadata
- `merges`: repository-associated direct merges and PR merges

### PrMerge

- `repo_id`: repository identity
- `pr_info`: number, URL, status, merge metadata

## Derived projection

### RepoInfo

A repository row derived only from the configured repo and the status where
`status.repo_id === repo.id`. Its optional PR fields are populated from an open
PR merge, otherwise a merged PR merge, in that matching status. No matching
status or PR means the PR fields are undefined.
