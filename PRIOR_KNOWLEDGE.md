# Prior Knowledge: Repository-scoped PR state in the Git panel

The project knowledge bases (`wiki/` and `docs/knowledge-base/`) were searched
for pull requests, repository identity, multi-repository behavior, Git panel
state, and workspace summaries.

## Relevant findings

1. The knowledge base has no existing topic specifically covering Git panel PR
   association or repository-scoped presentation state.
2. `interrupted-worktree-recovery.md` establishes a broader multi-repository
   invariant: partial per-repo outcomes must remain truthful, and success for
   one repository must not be generalized to the others. The same principle
   applies to PR links in a multi-repo workspace.
3. `issue-status-side-effects.md` documents that remote PR relationships are
   issue-level and that `pull_requests.workspace_id` is not reliably populated
   on creation. This warns against using issue/workspace-level remote PR data as
   a substitute for explicit local repository identity.
4. Existing workspace knowledge repeatedly distinguishes relationship truth
   from asynchronous/loading state. Missing repository-scoped data should
   remain unknown until its authoritative query arrives; a cached aggregate
   must not be projected onto a specific entity without an identity key.
5. The preferred verification pattern for small association policies is a pure
   helper plus focused unit tests, keeping rendering and asynchronous provider
   setup out of the truth table.

## Consequences for this task

- Treat `RepoBranchStatus.repo_id` as the association boundary.
- Do not apply workspace-summary PR fields to any repository row because the
  summary carries no repository ID.
- During branch-status loading, show no PR rather than a potentially false PR.
- Preserve the current within-repo precedence of open PR over merged PR.
- Record the shipped association invariant as reusable knowledge after review.
