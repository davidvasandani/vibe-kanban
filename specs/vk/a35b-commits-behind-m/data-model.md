# Data Model: Git Header Behind Status

## Existing inputs

### Repository metadata (`RepoWithTargetBranch`)

- `id`: stable join identity.
- `display_name`: preferred user-facing repository label.
- `name`: fallback repository label.
- `target_branch`: configured target; informational here because the backend
  already used it to calculate divergence.

### Branch status (`RepoBranchStatus`)

- `repo_id`: stable join identity.
- `commits_behind: number | null`: divergence from the configured target.

## Derived view model

`BehindHeaderStatus`:

- `visibleText: string`: compact header copy.
- `accessibleText: string`: complete singular/plural explanation.
- `entries: BehindEntry[]` (internal derivation):
  - `repoId: string`
  - `repoName: string`
  - `commitsBehind: number` (strictly greater than zero)

## Invariants

- Entries are joined by repository ID, never array index.
- Output entry order follows repository metadata order.
- Null, missing, zero, and negative values produce no entry.
- Repository naming depends on total workspace repository cardinality, not the
  number of positive entries.
