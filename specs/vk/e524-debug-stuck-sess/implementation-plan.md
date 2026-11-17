# Implementation plan: `e524-debug-stuck-sess`

> Revised after `/speckit.analyze`: the conditional write is one atomic
> statement, `terminalize_if_unlaunched`, that also requires a null `pgid`
> and no worker job. `plan.md` holds the current design.

Builds on `technical-spec.md` and `prior-knowledge.md`. Steps are in
dependency order.

## 1. Conditional completion write (`crates/db`)

File: `crates/db/src/models/execution_process.rs`.

- Add `ExecutionProcess::terminalize_if_unlaunched(pool, id, status,
  exit_code) -> Result<bool, sqlx::Error>`. It runs the same `UPDATE` as
  `update_completion` with `AND status = 'running'` and returns whether a
  row changed. Use a runtime `sqlx::query` (as the model already does
  elsewhere) so the offline `.sqlx` cache needs no regeneration.
- Tests, in the model's existing in-memory test module: a running row is
  updated and returns `true`; a terminal row is untouched and returns
  `false`.

## 2. In-flight start registry and drop guard (`crates/services`)

File: `crates/services/src/services/container.rs`.

- `static EXECUTION_STARTS: LazyLock<DashMap<Uuid, Instant>>`, beside
  `SESSION_ADMISSION`.
- `pub enum ExecutionStartState { NotStarting, Starting { age: Duration } }`
  and `pub fn execution_start_state(id: Uuid) -> ExecutionStartState`.
- `ExecutionStartGuard { execution_id, session_id, pool, armed }`:
  - `register(...)` inserts the entry;
  - `disarm(mut self)` removes the entry and clears `armed`;
  - `Drop` removes the entry and, if still armed, hands
    `fail_abandoned_start(pool, execution_id, session_id)` to
    `tokio::runtime::Handle::try_current()` (skipped silently if no
    runtime is alive, which only happens at process exit).
- `async fn fail_abandoned_start`: return if a worker job exists; else
  `terminalize_if_unlaunched(.., Failed, None)`; if it changed the row,
  log at `warn` and append a stderr `LogMsg` naming the abandonment. It
  takes no session admission (see `prior-knowledge.md`).
- In `start_execution_admitted`: register the guard right after
  `ExecutionProcess::create`; disarm it as soon as `dispatch_execution`
  returns, before the existing `Err` handling.
- Tests: disarm leaves a running row alone and clears the registry; drop
  marks the row `Failed` and clears the registry; drop leaves a row with a
  worker job `Running`; `execution_start_state` reports `Starting` while a
  guard is live.

## 3. Stop decision (`crates/services`)

Same file, so it is unit-testable without a `LocalContainerService`:

- `pub const ABANDONED_START_AFTER: Duration = 120 s`.
- `pub enum UnownedStopDecision { PreserveForBootRecovery, StillStarting,
  Terminalize }`.
- `pub fn unowned_stop_decision(pgid: Option<i64>, start:
  ExecutionStartState) -> UnownedStopDecision`: `pgid` set gives
  `PreserveForBootRecovery`; `Starting` younger than the bound gives
  `StillStarting`; everything else gives `Terminalize`.
- Tests: one per branch, plus the boundary (age equal to the bound
  terminalises).

## 4. Stop path (`crates/local-deployment`)

File: `crates/local-deployment/src/container.rs`, the ownerless branch of
`stop_execution` (after the adopted-pgid check).

- Re-read the row's `pgid` from the database; a failed or empty read falls
  back to the passed-in row, and if that is also unknown, to
  `PreserveForBootRecovery`.
- `PreserveForBootRecovery`: existing error, unchanged. Extend the comment
  to say the branch now covers launched rows only.
- `StillStarting`: `ContainerError::Other("Execution is still starting;
  try again shortly")`.
- `Terminalize`: `terminalize_if_unlaunched` with the requested status,
  `finish_msg_store`, `warn` log with the execution and session ids, `Ok`.

## 5. Visible stop failures (`crates/services`)

`try_stop`: log a failed `stop_execution` at `warn` instead of `debug`.

## 6. Verify

- `cargo test -p db -p services -p local-deployment` (targeted tests first,
  then the three crates).
- `pnpm run backend:check`, `pnpm run lint`, `pnpm run format`.
- No TypeScript type changes are expected; confirm with
  `pnpm run generate-types:check`.

## 7. Review, knowledge, ship

- Codex review of the diff; fix confirmed findings and re-verify.
- Knowledge base: extend `docs/knowledge-base/interrupted-worktree-recovery.md`
  (the load-bearing-row rule now has a never-launched exception) and
  `wiki/agent-process-lifecycle.md` (abandoned starts, the guard, the
  hung-start residual); refresh both indexes.
- PR against `main`, merge, then confirm on the live coordinator that
  `5cc7e118` is terminal and the session accepts a turn.
