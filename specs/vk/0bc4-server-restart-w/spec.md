# Feature Specification: Never discard uncommitted worktree work on restart

**Feature dir**: `specs/vk/0bc4-server-restart-w/`
**Status**: Draft

## Summary

A running agent session lost a full session's uncommitted work when the Vibe
Kanban server restarted underneath it. The worktree came back clean at the base
commit and nothing in the log recorded the loss. Because the server auto-restarts
on every self-deploy, this is a routine path, not a rare crash.

Investigation established that the specific code that destroyed the tree —
delete-and-recreate on drifted git worktree linkage — was fixed 20 minutes
*before* the incident by #151, which the incident's own restart was deploying.
This feature therefore closes the two data-loss paths that #151 did **not**
address: work-preservation on shutdown is skipped whenever stopping the process
reports an error, and the startup orphan sweep deletes workspace directories with
no check for unsaved work and effectively no logging.

The user-visible goal is simple: a restart must never cost an agent its
uncommitted work, and if work ever is at risk, the system must say so out loud.

## User Stories

- As an **agent running a long task**, I want my uncommitted and staged changes
  to survive a server restart, so that a routine self-deploy does not destroy
  hours of work I cannot recover.
- As a **developer operating the server**, I want any decision to delete a
  workspace directory to be logged with the path, the reason, and the action
  before it happens, so that I can reconstruct what occurred without resorting
  to filesystem mtimes and reflogs.
- As a **developer diagnosing a suspected loss**, I want a failure to preserve
  work to be reported loudly rather than silently swallowed, so that I learn
  about it at the time rather than when the next session finds an empty tree.
- As a **user with a genuinely finished workspace**, I want dead workspace
  directories still to be reclaimed, so that the worktree base directory does
  not grow without bound.

## Functional Requirements

- **FR-1**: When the server shuts down, it must attempt to preserve the
  uncommitted work of every interrupted coding-agent and cleanup-script process,
  regardless of whether stopping that process succeeded. Failure to stop a
  process must not cause its work to go unpreserved — these are independent
  concerns, and the stop-failure case is the one most likely to leave unsaved
  work on disk.
- **FR-2**: A failure to preserve work must be reported at error severity, and
  must identify the workspace and the repositories affected, stating that
  uncommitted work may be at risk.
- **FR-3**: Before deleting a workspace directory it has judged abandoned, the
  system must establish that the directory holds no uncommitted or untracked
  work, and must retain it if it does.
- **FR-4**: If the system cannot determine whether such a directory holds unsaved
  work, it must retain the directory. An error must never be treated as evidence
  that a directory is empty.
- **FR-5**: Before performing a destructive action on a workspace directory, the
  system must log the path, the reason the directory was selected for that
  action, and the action being taken, at a severity visible in normal operation.
- **FR-6**: The system must continue to reclaim workspace directories that are
  genuinely abandoned and hold no unsaved work.
- **FR-7**: The system must not report a destructive action as successful when it
  did not actually take place.

## Out of Scope

- Re-implementing the protections delivered by #151: repair-first worktree
  recreation, moving valuable directories aside to `.recovered-<epoch>`, and the
  expiry-sweep cleanliness guard. These already exist and are working.
- Surfacing the loss in the user interface. The existing code comment at the
  forced-reset path notes that full UI surfacing is tracked separately in
  VAS-104; this feature is a different path that produces the same outcome, and
  it links to that work rather than duplicating it.
- Changing the `force_when_dirty` reset contract, which the incident report ruled
  out and which investigation confirmed behaves as designed: it already refuses
  when dirty and already warns before discarding.
- Changing what triggers the server restart.
- Binding pooled worktree administrative directory names to workspace ids.
  Investigation showed cleanup resolves these by path rather than by name, so
  a recycled name cannot cause one task's teardown to affect another's tree; the
  change would add risk for no benefit.
- Moving the worktree base directory off the OS temp directory. This is a real
  systemic contributor (temp reaping produces exactly the drifted state that
  drives recreation) but is a larger, riskier change than this fix warrants.

## Acceptance Criteria

- [ ] **AC-1**: A coding-agent process that cannot be stopped cleanly at shutdown
      still has its uncommitted work preserved, in every repository of the
      workspace that has changes.
- [ ] **AC-2**: A workspace directory that looks abandoned but contains
      uncommitted or untracked changes survives the startup sweep, and the reason
      it was retained is logged.
- [ ] **AC-3**: A workspace directory that looks abandoned and is clean is still
      removed by the startup sweep.
- [ ] **AC-4**: In a workspace holding several repositories, unsaved work in any
      one of them is sufficient to retain the whole workspace — including when
      the changes are in a repository other than the first.
- [ ] **AC-5**: Reproducing the reported scenario end-to-end — create a
      workspace, make uncommitted and staged changes, restart the server, resume —
      leaves the changes intact, and the log shows what the cleanup routines
      decided.
- [ ] **AC-6**: `cargo test --workspace`, `pnpm run check`, and `pnpm run lint`
      pass.

## Clarifications

All three questions were resolved from the codebase rather than by assumption.
One resolution inverts the original assumption and is load-bearing.

### C-1: Do not mark the execution row `Interrupted` when no child is found

**Resolved: leave the row `Running`. Changing it would remove a safety net.**

The startup routine `cleanup_orphan_executions` selects rows that are still
`Running` and preserves their work **unconditionally** — it is not gated on the
kill succeeding. It is precisely the early return described in this question
(returning before the completion update, leaving the row `Running`) that causes
the next startup to find the process and capture its work.

So marking the row `Interrupted` at shutdown would make it invisible to
`find_running`, deleting the backstop that currently rescues this case. The
knowledge base's "do not leave a killed execution `Running`" rule applies to the
startup path, where it is already implemented correctly, and where the row is
marked `Interrupted` *after* capture is attempted.

This also **corrects the severity assessment for FR-1**: the shutdown gap is
largely backstopped today, so it is a defence-in-depth fix rather than the sole
cause of the reported loss. It still matters, because the backstop only works if
there is a next startup, and because it depends on an early-return side effect
rather than on intent.

### C-2: No running-execution check is possible for abandoned directories

**Resolved: structurally vacuous; the uncommitted-work check is the real
protection.**

A directory is judged abandoned exactly when no workspace record refers to it.
Execution processes belong to sessions, which belong to workspaces, and both
relationships cascade on delete. If no workspace record exists for a directory,
no execution process record can exist for it either — so there is never a running
execution to find. The report's ask cannot be implemented as stated for this
path.

The equivalent protection for workspaces that *are* still recorded already
exists in the expiry sweep, which #151 guarded. For this path, FR-3 and FR-4 are
the protection.

### C-3: Use the untracked-counting definition; do not reconcile the other

**Resolved: use the stricter definition locally; record the divergence.**

The definition that counts staged and untracked changes is the one already used
by the expiry-sweep guard and by #151's preserve-aside check, so using it here
keeps all three retention decisions consistent. The looser definition is only
reachable through the non-forced reset guard, which is explicitly out of scope;
reconciling it would change that contract. The divergence is recorded for the
knowledge base instead.

## Open Questions

None blocking. The divergence noted in C-3 and the systemic issues listed under
Out of Scope are recorded for follow-up rather than resolved here.
