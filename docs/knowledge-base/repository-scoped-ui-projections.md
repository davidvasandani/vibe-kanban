# Repository-scoped UI projections

Tags: `vk/63e0-git-panel-shows`

## Identity before enrichment

Workspace and issue summaries often aggregate child state and omit the child
identity. They are suitable for workspace-level badges or ordering, but they are
not evidence that a fact belongs to a particular repository row. A child row may
consume enrichment only when the source carries an identity that matches it.

This matters most during loading: a cached workspace PR is plausible and fast,
but projecting it across repository rows makes unrelated repositories appear to
have shipped, suppresses their actions, and creates links into the wrong repo.
Unknown is the truthful fallback until scoped data arrives.

## Git panel association boundary

For local Git panel rows, `RepoBranchStatus.repo_id` is the association key.
`RepoBranchStatus.merges` is already repository-owned, so the row projection
joins configured `repo.id` to `status.repo_id` and selects PRs only after that
match. Workspace summary PR fields have no `repo_id` and must not be used as a
row fallback, even in a single-repository workspace.

Remote/issue-level pull-request lists do not repair this gap by themselves.
Manual remote PR discovery needs an explicit provider/repository-to-local-repo
association; parsing a URL or assuming the only configured repo re-derives
identity and will diverge across providers, renamed remotes, and multi-repo
workspaces.

## Verification pattern

Keep deterministic association policy in a pure helper and test:

- mixed siblings where exactly one owns the enriched fact;
- absent/loading enrichment;
- source arrays in a different order from the rows; and
- any existing within-entity precedence rule (for Git panel PRs, open before
  merged).

Import the pure helper directly in focused tests. Importing a full container can
pull in providers, virtual modules, and unrelated runtime configuration, making
an association unit test brittle without increasing contract coverage.
