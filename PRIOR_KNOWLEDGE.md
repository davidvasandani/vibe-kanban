# Prior Knowledge: Server restart wipes uncommitted worktree work

The knowledge base is populated (17 pages). Exactly one page is directly on
point, and it is unusually valuable here: it was written by the commit that
introduced the very mechanism this task must repair.

## `interrupted-worktree-recovery` — the governing page

`docs/knowledge-base/interrupted-worktree-recovery.md`, tags `959a-restart-rewinds`,
authored by `adf29235` ("Preserve interrupted agent work across restarts", #122)
— the commit that added `commit_interrupted_wip`.

The page states the lifecycle invariant this task is enforcing:

> Startup recovery kills an unadopted orphan writer first, then snapshots dirty
> coding-agent/cleanup-script repositories with `WIP: run interrupted by
> vibe-kanban shutdown`, and only offers the process for auto-resume after
> capture succeeds.

> **Snapshot failures are not a reason to leave a killed execution `Running`:**
> mark the dead row `Interrupted`, exclude it from the recovered/auto-resume
> list, and log the failure with execution/repository context.

**The documented invariant and the shipped code disagree.** The page describes
kill-then-snapshot as a sequence; the code makes the snapshot *conditional on the
kill succeeding* (`container.rs:2668`, `commit_interrupted_wip` inside the `else`).
When `stop_execution` returns `Err("Child process not found for execution")` —
the ordinary case at restart — no snapshot is attempted and the row is left
`Running`, violating both sentences above. This task closes that gap rather than
inventing a new policy: the intended behaviour was already written down.

Two more directly reusable rules from the same page:

- **Multi-repository partial failure.** "WIP capture is best-effort across
  repositories. Attempt every dirty repo even if one commit fails … refresh
  every repo's `after_head_commit` before returning an aggregate error."
  `commit_interrupted_wip` already implements this correctly; preserve it when
  changing the call site.
- **Verification pattern.** "Keep the reset decision as a small pure helper with
  a truth-table unit test." This is the established shape for safety predicates
  in this codebase — `reset_would_discard_uncommitted_work`
  (`crates/services/src/services/container.rs:73`) plus
  `dirty_git_reset_requires_explicit_force` (`:1774`) is the working example.
  New retain/destroy decisions should follow it, because the crates that own the
  destructive code (`workspace-manager`, `worktree-manager`) have **no test
  infrastructure at all**, so a pure helper is the only cheaply testable unit.

The page's closing line is a direct instruction for stage 11 of this pipeline:

> Independent review should explicitly probe killed-orphan failure state and
> multi-repository partial commits; both are easy to miss in the happy path.

The reported incident *is* the killed-orphan failure state. It was flagged as
easy to miss, and it was missed.

## Reset boundary (same page) — bounds the non-goals

The page fixes the `force_when_dirty` contract: non-forced dirty reset must
reject; `force_when_dirty=true` is explicit authorisation for `reset --hard` +
`clean -fd`. The incident report already ruled this path out, and the page
confirms it is behaving as designed. **Do not touch it.**

## Related but not directly applicable

- `worktree-formatting-prerequisites` (`7243-make-frontend-fo`) — fail-before-mutation
  preflight checks. The general stance (validate before mutating) is the same
  instinct behind guarding the orphan sweep, but no shared code.
- `issue-status-side-effects` (`2f63-auto-archive-wor`, `f464-vk-workspace-mgm`) —
  terminal-status workspace archiving. Relevant only as background for *why*
  expiry accelerates to 1h on archive, which is the pressure that made the
  expiry-sweep guard in #151 necessary.

## Knowledge gaps this task will need to fill

Nothing in the KB covers:

- The **orphan** sweep (`cleanup_orphan_workspaces`) as distinct from the
  **expiry** sweep. The KB and the #151 work both address expiry; the orphan
  sweep is undocumented and unguarded.
- That orphan status is decided by an **exact, un-canonicalised string match**
  on `container_ref`, making it fragile to path normalisation drift.
- That the two cleanliness helpers disagree: `is_container_clean` counts
  untracked files, `check_worktree_clean` (git2) does not.

These are candidates for the stage-12 knowledge update.
