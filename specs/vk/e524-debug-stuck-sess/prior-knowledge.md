# Prior knowledge: `e524-debug-stuck-sess`

Distilled from the two project knowledge bases (`wiki/` and
`docs/knowledge-base/`), read-only. Each entry says what the page
establishes and what it means for this task.

## Constraints this change must respect

### The `Running` row left by a failed stop is load-bearing

Source: `docs/knowledge-base/interrupted-worktree-recovery.md`.

`stop_execution` returns "child process not found" without updating the row
when there is no child handle and no adopted pgid. The page says not to
"fix" this: the next boot's `cleanup_orphan_executions` selects through
`find_running` and snapshots uncommitted work unconditionally, so marking
the row terminal on the shutdown path removes the last safety net.

**Consequence.** The stop path may not terminalise every ownerless row. It
may only terminalise a row that provably never launched, because such a row
has no work to snapshot. The first draft of the technical spec ignored this
and has been corrected.

### An execution row is not launch evidence

Source: `wiki/issue-comment-delivery.md`.

Durable launch evidence is `pgid IS NOT NULL` (never cleared), an
`execution_worker_jobs.accepted_at`, or
`coding_agent_turns.agent_session_id`. A row with none of these proves
nothing ran.

**Consequence.** This gives the criterion the constraint above needs. A
`Running` row with a null `pgid`, no worker job row, no child in
`child_store` and no adopted pgid never launched. The stuck execution
`5cc7e118` matches exactly.

### Dropping a request future drops the server work

Source: `docs/knowledge-base/request-independent-workspace-creation.md`.

If an Axum handler awaits a slow sequence, dropping the browser request can
drop the server future at any await. Process startup can insert session and
execution state and then be interrupted before returning. Workspace
creation solved this by accepting first and running in a Tokio-owned task.

**Consequence.** Confirms the root-cause mechanism: `follow_up` awaits the
start inline. Moving follow-up starts into an owned task is the fuller fix
and is out of scope here; a drop guard closes the same hole with less
surface.

### Session admission

Source: `wiki/agent-process-lifecycle.md`, "Session admission and
finalization".

`with_session_admission` serialises initial startup, UI/MCP follow-ups and
continuations per session. Spawned tasks acquire their own guard.

**Consequence.** A start that hangs (rather than drops) still holds the
session's admission lock. The drop guard's cleanup task must not take
session admission, or it could deadlock behind the very start it is
cleaning up. It only does a conditional row update.

### Worker ownership and reconciliation

Sources: `docs/knowledge-base/clustered-workspace-execution.md`,
`wiki/coordinator-restart-handoff.md`.

A `Running` row that has a worker job belongs to the worker; reconciliation
is the authority, and boot orphan cleanup skips it. A failed dispatch must
terminalise its worker-job record. Ordinary agents are never silently
marked complete.

**Consequence.** Neither the guard nor the stop path may touch a row that
has a worker job. The terminal status for an abandoned start is `Failed`
(guard) or the caller's requested status (stop), never `Completed`.

### Teardown only stops `Running` executions

Source: `wiki/agent-process-lifecycle.md`.

`try_stop` calls `stop_execution` only for rows whose status is `Running`.

**Consequence.** `try_stop` is the UI Stop path, so the fix belongs in
`stop_execution`; no route change is needed.

## Gaps

No page covers an execution abandoned between row creation and dispatch, or
the fact that `try_stop` hides stop failures at `debug`. Both are new
knowledge for the final stage.
