# Implementation Plan: A turn that never started can always be stopped

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context
Rust workspace, Tokio, Axum, SQLx on SQLite. Three crates change: `db`
(models), `services` (the `ContainerService` trait and its shared start
path), `local-deployment` (`LocalContainerService`, which is both the local
and the cluster-coordinator implementation). No frontend, schema, API or
generated-type change. Compile-time `sqlx::query!` macros need a regenerated
offline cache; the one new query is a runtime `sqlx::query`, which
`crates/db/src/models/execution_process.rs` already uses elsewhere.

## Architecture & Approach

### FR-6, FR-7: one atomic write (`crates/db/src/models/execution_process.rs`)
`ExecutionProcess::terminalize_if_unlaunched(pool, id, status)`, beside
`update_completion`: a single `UPDATE` that sets the terminal status only
while the row is `running`, its `pgid` is null, and no
`execution_worker_jobs` row exists for it. It returns whether a row
changed. The guard and the stop path both write through it, so neither can
overwrite a terminal state or touch a turn that launched, and there is no
gap between checking the evidence and writing. (Revised after
`/speckit.analyze` findings 1, 2 and 4.)

### FR-3: abandoned-start guard (`crates/services/src/services/container.rs`)
- `EXECUTION_STARTS: LazyLock<DashMap<Uuid, Instant>>`, beside
  `SESSION_ADMISSION`.
- `ExecutionStartGuard`, registered in `start_execution_admitted` right
  after `ExecutionProcess::create` and disarmed as soon as
  `dispatch_execution` returns. The existing `Err` branch still marks the
  row `Failed`.
- Dropped while armed, the guard spawns `fail_abandoned_start`: it calls
  `terminalize_if_unlaunched(.., Failed)` and, when that changed the row,
  appends a stderr line through `execution_process::append_log_message`
  and logs at `warn`. A turn with a process group or a worker job is left
  alone (FR-6).

### FR-1, FR-4, FR-5, FR-6: stop decision
- In `services`: `ExecutionStartState`, `execution_start_state(id)`,
  `ABANDONED_START_AFTER` (120 s), and the pure
  `unowned_stop_decision(pgid, start_state)` returning
  `PreserveForBootRecovery | StillStarting | Terminalize`. Keeping it pure
  makes every branch unit-testable; `LocalContainerService` has no test
  harness for `stop_execution`.
- In `crates/local-deployment/src/container.rs::stop_execution`, the
  branch reached when there is no worker job, no `child_store` entry and no
  adopted pgid re-reads the row and applies the decision:
  - `PreserveForBootRecovery` (pgid set): the existing error, row untouched.
  - `StillStarting`: an error that says so.
  - `Terminalize`: `terminalize_if_unlaunched` with the requested status,
    `finish_msg_store`, a `warn` log, `Ok`. If the write changed nothing
    and the row is still `running`, launch evidence appeared in between,
    so it falls back to the existing error.

FR-2 follows: `has_running_coding_agent_for_session` reads the row status.
A hung start still holds `SESSION_ADMISSION`, which is the exception FR-2
names.
FR-9 follows: the guard writes `Failed`; stop writes the caller's status,
which is `Killed` (user) or `Interrupted` (shutdown).

### FR-8: `ContainerService::try_stop`
The swallowed `stop_execution` error is logged at `warn`, not `debug`.

### Tests
- `db`: `terminalize_if_unlaunched` on an unlaunched running row, a
  terminal row, a row with a `pgid`, and a row with a worker job.
- `services`: guard disarm, guard drop, guard drop with a worker job,
  registry state, and each `unowned_stop_decision` branch plus the bound.

### Launch boundary (added after Codex review)
The registry entry carries a phase: `Preparing`, `Launching`, `Stopped`,
`LaunchInterrupted`. Every transition happens under the map's entry lock.

- The guard is registered **before** `ExecutionProcess::create`, because
  `create` inserts the row and then awaits more work.
- `begin_execution_launch(id)` moves `Preparing` to `Launching` and is
  called in `crates/local-deployment/src/container.rs` immediately before
  `ExecutionWorkerJob::create_pending` (cluster) and before the warm server
  is claimed or a process is spawned (local). It returns `false` if a stop
  already ended the execution, and the start then gives up.
- `resolve_unowned_stop(id, pgid)` decides and, when it terminalises a
  preparing start, sets `Stopped` in the same step. A start that is
  `Launching` answers `StillStarting` whatever its age.
- A guard dropped while `Launching` sets `LaunchInterrupted` and leaves the
  row `Running`: a process may have run before its `pgid` was recorded, so
  boot recovery must still see the row.
- When dispatch returns `Err` for a start that was stopped before launch,
  `start_execution_admitted` leaves the stop's status in place instead of
  overwriting it with `Failed`.

## Data Model
No change. No new table, column or migration.

## Contracts
No change. `POST /api/execution-processes/{id}/stop` and
`POST /api/workspaces/{id}/execution/stop` keep their shapes; the first now
succeeds for a never-launched turn where it used to answer 500.

## Research Notes
See `./research.md`.

## Constitution Check
Checked against `.specify/memory/constitution.md` (0.56.2).

- **II Test the contract**: tests assert the row's resulting status and the
  stop decision, not internals.
- **III / VI**: one guard and one branch in existing code paths. No
  sweeper, no job framework.
- **XI**: stop failures become visible at `warn`.
- **XII**: the guard's cleanup takes no session-admission lock and only
  performs a compare-and-set write.
- **XV**: a launched row is still left `Running` for boot-time work
  preservation.
- **XVIII / LVI**: a row with a worker job is never terminalised from the
  coordinator. The 120 s bound is not a reaper: it applies only on an
  explicit stop, and only to a start that was never dispatched.
- **XXVIII (as refined)**: the writer of the pre-owner `running` state now
  terminalises it on abandonment, and an explicit stop records an outcome.

No deviation.

## Risks & Dependencies
- A start that hangs past the bound and is stopped can no longer launch:
  the stop revokes it at the launch boundary. It still holds the session's
  admission lock until it returns, so a new turn waits behind it (the FR-2
  exception). A timeout on dispatch preflight would close that and is left
  as a follow-up.
- `LaunchInterrupted` entries are never removed. They arise only when a
  caller is dropped mid-launch, and they are cleared by a restart.
- `Drop` runs outside a Tokio runtime only at process exit; the guard then
  skips cleanup and boot `cleanup_orphan_executions` covers the row.
- The live stuck turn clears on the deployment restart, not through this
  code path.
