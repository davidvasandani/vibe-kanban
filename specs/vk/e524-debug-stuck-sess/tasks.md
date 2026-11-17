# Tasks: A turn that never started can always be stopped

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes. Three tasks share `crates/services/src/services/container.rs` and
are therefore serial.

## Phase 1: Foundation
- [x] T001 Add `ExecutionProcess::terminalize_if_unlaunched` and its tests
      (unlaunched running row updated; terminal row, row with a `pgid`, and
      row with a worker job untouched) in
      `crates/db/src/models/execution_process.rs` (FR-6, FR-7)

## Phase 2: Core
- [x] T002 Add the `EXECUTION_STARTS` registry, `ExecutionStartState`,
      `execution_start_state`, `ExecutionStartGuard` and
      `fail_abandoned_start` in `crates/services/src/services/container.rs`
      (FR-3, FR-6, FR-7, FR-9; depends on T001)
- [x] T003 Register the guard after `ExecutionProcess::create` and disarm it
      when `dispatch_execution` returns, in `start_execution_admitted` in
      `crates/services/src/services/container.rs` (FR-3; depends on T002)
- [x] T004 Add `ABANDONED_START_AFTER`, `UnownedStopDecision` and
      `unowned_stop_decision` in `crates/services/src/services/container.rs`
      (FR-4, FR-5, FR-6; depends on T002)
- [x] T005 Apply the decision in the ownerless branch of `stop_execution` in
      `crates/local-deployment/src/container.rs` (FR-1, FR-2, FR-4, FR-5,
      FR-6, FR-9; depends on T001, T004)
- [x] T006 Log a failed stop at `warn` in `try_stop` in
      `crates/services/src/services/container.rs` (FR-8; after T004, same
      file)

## Phase 3: Validation
- [x] T007 Add guard and registry tests (disarm, drop, drop with a worker
      job, state while live) and `unowned_stop_decision` tests (each branch
      and the bound) in the test module of
      `crates/services/src/services/container.rs` (depends on T003, T004)
- [x] T008 Run `cargo test -p db -p services -p local-deployment`,
      `pnpm run backend:check`, `pnpm run generate-types:check`,
      `pnpm run lint` and `pnpm run format`; fix anything they report
      (depends on T005, T006, T007)

## Phase 4: After merge
- [ ] T009 On the live coordinator, confirm execution
      `5cc7e118-c02b-4c33-ad7e-6a864fb53ad7` is terminal and session
      `08279ebe-f674-4d02-a448-866c618bed15` accepts a turn (no file
      change; depends on the deployment)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
