# Interrupted worktree recovery and reset safety

Tags: `959a-restart-rewinds`

## Lifecycle invariant

An execution's Git snapshot and its `execution_process_repo_states` metadata
must describe the same durable repository state. Startup recovery kills an
unadopted orphan writer first, then snapshots dirty coding-agent/cleanup-script
repositories with `WIP: run interrupted by vibe-kanban shutdown`, and only
offers the process for auto-resume after capture succeeds.

Snapshot failures are not a reason to leave a killed execution `Running`: mark
the dead row `Interrupted`, exclude it from the recovered/auto-resume list, and
log the failure with execution/repository context.

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
