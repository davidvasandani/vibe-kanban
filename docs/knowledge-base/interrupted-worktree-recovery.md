# Interrupted worktree recovery and reset safety

Tags: `959a-restart-rewinds`, `0bc4-server-restart-w`

## Lifecycle invariant

An execution's Git snapshot and its `execution_process_repo_states` metadata
must describe the same durable repository state. Startup recovery kills an
unadopted orphan writer first, then snapshots dirty coding-agent/cleanup-script
repositories with `WIP: run interrupted by vibe-kanban shutdown`, and only
offers the process for auto-resume after capture succeeds.

Snapshot failures are not a reason to leave a killed execution `Running`: mark
the dead row `Interrupted`, exclude it from the recovered/auto-resume list, and
log the failure with execution/repository context.

## Preservation is never conditional on teardown succeeding

Stopping a process and preserving its work are independent concerns. Gate the
second on the first and you skip preservation in precisely the case it exists
for: `stop_execution` returns "child process not found" whenever the child died
with (or just before) the server, which is the *ordinary* shape of a restart —
not an exotic failure. Attempt the WIP snapshot for every non-persistent
process, on both the success and the error branch, ordered after the stop
attempt so a live writer has been signalled first.

The same rule generalises: no teardown failure (unreachable repo, failed
metadata cleanup, unkillable child) may skip preservation of that unit's
uncommitted work.

## The `Running` row left by a failed stop is load-bearing

`stop_execution` returns before `update_completion` when no child handle and no
adopted pgid exist, leaving the row `Running`. This looks like a bug and is
tempting to "fix". Do not.

That row state is exactly what makes the next boot's `cleanup_orphan_executions`
(which selects via `find_running` and snapshots unconditionally) rescue the
session. Marking it terminal at shutdown hides it from that sweep and deletes
the last safety net. The "don't leave a killed execution `Running`" rule above
applies to the *startup* path, which marks the row terminal only **after**
attempting capture — not to the shutdown path.

Corollary for triage: because that backstop exists, a shutdown-side preservation
gap usually costs nothing visible, which is why it survived review. It still
matters — the backstop needs a next startup to happen at all.

## Multi-repository partial failure

WIP capture is best-effort across repositories. Attempt every dirty repo even
if one commit fails. Because successful commits cannot be rolled back safely,
refresh every repo's `after_head_commit` before returning an aggregate error.
This makes partial success truthful: successful repo HEADs are durable and
recorded, failed repos retain their actual current HEAD, and the process is not
auto-resumed as though capture fully succeeded.

## Reset boundary

`reset_session_to_process` combines filesystem reconciliation with process stop
and soft-dropping execution history. A dirty non-forced Git reset must fail
before all three operations; merely allowing Git reconciliation to skip while
continuing database cleanup creates a partial reset.

The existing request flags are the contract:

- `perform_git_reset=true`, dirty, `force_when_dirty=false`: reject with an
  actionable error and preserve files/history.
- `force_when_dirty=true`: explicit authorization for `git reset --hard` plus
  `git clean -fd` (ignored files remain outside this cleanup).
- `perform_git_reset=false`: retain the existing non-Git-reset semantics.

## Verification pattern

Keep the reset decision as a small pure helper with a truth-table unit test, and
exercise Git reconciliation separately for dirty skipped and forced reset paths.
Independent review should explicitly probe killed-orphan failure state and
multi-repository partial commits; both are easy to miss in the happy path.
