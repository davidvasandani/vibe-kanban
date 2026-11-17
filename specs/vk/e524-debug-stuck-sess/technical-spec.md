# Technical spec: an execution that was never dispatched cannot be stopped

> Revised after `/speckit.analyze`: the conditional write is one atomic
> statement, `terminalize_if_unlaunched`, that also requires a null `pgid`
> and no worker job. `plan.md` holds the current design.

Task: `e524-debug-stuck-sess` (branch `vk/e524-debug-stuck-sess`)

## Incident

Workspace `3f2d3514-eada-4a84-9d0b-86182f6afef8` (Ohana / VAS-1059), session
`08279ebe-f674-4d02-a448-866c618bed15`, Codex, placed on worker think4.

Execution `5cc7e118-c02b-4c33-ad7e-6a864fb53ad7` was created at
2026-11-17T07:12:00.313Z by a UI "Continue" follow-up and has been `running`
ever since. The user cannot stop it, and a hard refresh changes nothing.

Evidence gathered on 2026-11-17 (all read-only except one Stop call):

| Observation | Source |
| --- | --- |
| Row is `running`, `pgid` null, `updated_at == created_at` | `GET /api/execution-processes/{id}` |
| No worker job exists for it; the previous turn has one (`completed`) | `GET /api/execution-processes/{id}/worker-job` |
| think4 never received a dispatch for it; no process exists | worker journal, `ps` on think4 |
| Its raw log is unreadable (nothing was ever written) | `search_session_messages` |
| Coordinator has run without restart since 06:45:18Z | `systemctl show vibe-kanban-dev.service` on think2 |
| Coordinator logged the follow-up at 07:11:55Z and no start error after | coordinator journal |
| `POST /api/execution-processes/{id}/stop` returns 500, `Child process not found for execution`; row stays `running` | reproduced at 21:06:29Z |
| The UI Stop (`POST /api/workspaces/{id}/execution/stop`) left no log line at all | coordinator journal |

## Root cause

Two defects combine.

### 1. A start can be abandoned after its row is created

`start_execution_admitted` (`crates/services/src/services/container.rs`)
inserts the `ExecutionProcess` row as `Running`, then awaits
`dispatch_execution`. Its failure handling runs only when that future
returns `Err`. The whole chain runs inline in the HTTP handler
(`routes/sessions/mod.rs::follow_up`), so if the future is dropped (the
client disconnects) or never completes, nothing marks the row terminal.

In cluster mode `dispatch_execution`
(`crates/local-deployment/src/container.rs`) awaits several things before
`ExecutionWorkerJob::create_pending`: the placement and worker lookups,
`resolve_org_env_vars` (secret resolution), and the MCP server snapshot.
An abandonment in that window leaves a `Running` row with no worker job,
no child and no pgid. That is exactly the state observed.

The journal cannot tell a dropped future from a hung one. The fix has to
cover both.

### 2. Nothing can stop an ownerless row

`LocalContainerService::stop_execution` finds no worker job, no child in
`child_store` and no adopted pgid, and returns
`Child process not found for execution` **without** updating the row. The
comment there calls this load-bearing: the next boot's
`cleanup_orphan_executions` must still see the row to snapshot WIP. That
reasoning holds for a row whose process launched. It does not hold for a
row that never launched (null `pgid`, no worker job): there is no work to
snapshot, and only a coordinator restart ever clears it.

`try_stop` then hides the failure: it logs the error at `debug` and the
route returns 200, so the UI Stop appears to do nothing.

## Goals

1. Stop succeeds on an ownerless, abandoned execution and records the
   requested terminal status.
2. A start dropped before dispatch terminalises its own row, so the state
   does not arise from a client disconnect.
3. A stop failure is visible in the coordinator log at `warn`.
4. No change to the stop behaviour of an execution that has an owner
   (worker job, child, or adopted pgid), or of a start that is genuinely
   still in flight.

## Non-goals

- Running the start outside the request future.
- A periodic sweeper for ownerless rows.
- Any frontend change. The UI already follows the authoritative process
  stream, which clears Stop once the row is terminal.
- Manually editing the live row. The deploy of this change restarts the
  coordinator, and boot `cleanup_orphan_executions` marks `5cc7e118`
  `interrupted` because it has no worker job.

## Design

### In-flight start registry (`services` crate)

A process-wide `DashMap<Uuid, Instant>` keyed by execution id, owned by an
RAII `ExecutionStartGuard`:

- Registered in `start_execution_admitted` immediately after
  `ExecutionProcess::create` succeeds.
- Disarmed and removed when `dispatch_execution` returns, `Ok` or `Err`.
  The existing `Err` path already marks the row `Failed`.
- On drop while armed (the future was dropped), it removes the entry and
  spawns a task that marks the row `Failed` **only if** the row is still
  `Running` and has no worker job, then appends a stderr log line saying
  the start was abandoned before dispatch. A row that already has a worker
  job is left alone: the worker may own a process, and cluster
  reconciliation is the authority for it.

`execution_start_state(id) -> NotStarting | Starting { age }` exposes the
registry to the stop path.

### Stop path (`local-deployment` crate)

The ownerless branch of `stop_execution` (no worker job, no child, no
adopted pgid) splits on durable launch evidence, re-read from the database:

- **`pgid` is set**: the execution launched. Behaviour is unchanged: return
  the error and leave the row `Running`, because the next boot's
  `cleanup_orphan_executions` must still find it to snapshot WIP (see
  `prior-knowledge.md`).
- **`pgid` is null**: the execution never launched, so there is no work to
  snapshot.
  - `Starting` with age below `ABANDONED_START_AFTER` (120 s): return an
    error saying the execution is still starting.
  - Otherwise: terminalise. Conditionally update the row to the requested
    status (only while it is still `Running`), finish its `MsgStore`, and
    return `Ok`.

The conditional update needs a new
`ExecutionProcess::terminalize_if_unlaunched` so a racing exit monitor
or tracker that already wrote a terminal state is never overwritten.

### Logging

`try_stop` logs a failed `stop_execution` at `warn`.

## Risks and residuals

- **Hung start that later resumes.** If a start hangs past 120 s, the user
  stops it, and the start then proceeds, it dispatches a turn under a
  terminal row. Dispatch preflight has no timeout today; adding one is a
  larger change and is left as a follow-up. There is no mitigation in
  this change; the residual is recorded in the knowledge base.
- **Drop after the worker accepted.** The guard leaves a row with a worker
  job untouched, so this case is exactly as before.
- **Local child spawned, future dropped before `child_store` insert.** The
  guard marks the row `Failed`; the child handle's `kill_on_drop` reaps
  the direct child, as it does today.

## Verification

- Unit tests for the guard: disarm leaves the row alone; drop marks a
  `Running` row `Failed`; drop leaves a row with a worker job `Running`.
- Unit tests for the stop path: a never-launched ownerless row is
  terminalised with the requested status; a launched ownerless row (pgid
  set) still errors and stays `Running`; a fresh in-flight start is
  refused; a stale in-flight start is terminalised; an already-terminal
  row is not overwritten.
- `cargo test` for `db`, `services`, `local-deployment`; `pnpm run check`,
  `pnpm run lint`, `pnpm run format`.
- After deploy: confirm `5cc7e118` is terminal and the session accepts a
  new turn.
