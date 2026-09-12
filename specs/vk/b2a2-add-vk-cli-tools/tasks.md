# Tasks: CLI Tools in Workspace Sessions

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may run together within their layer.

## Phase 1: Shared PATH contract

- [x] T001 Add the canonical host-first, missing-directory-safe managed CLI PATH
  augmentation helper and focused ordering/preservation/deduplication tests in
  `crates/utils/src/shell.rs` and update the helper contract comment in
  `crates/utils/src/assets.rs`.

## Phase 2: Execution-host integrations

- [x] T002 [P] Replace local managed-execution inline PATH assembly with the
  shared helper and retain focused environment-contract coverage in
  `crates/local-deployment/src/container.rs`. Depends on T001.
- [x] T003 [P] Add execution-host PATH augmentation to the local workspace
  terminal branch without changing machine-scoped login PTYs in
  `crates/server/src/routes/terminal.rs`. Depends on T001.
- [x] T004 [P] Add worker-local PATH augmentation to clustered managed execution
  for both raw commands and executor actions, with focused raw-command evidence
  in `crates/worker/src/execution.rs`. Depends on T001.
- [x] T005 [P] Add worker-local PATH augmentation to clustered workspace
  terminal spawning and extend terminal helper/process coverage in
  `crates/worker/src/terminal.rs`. Depends on T001.

## Phase 3: Verification and consistency

- [x] T006 Run focused `utils`, `local-deployment`, and `worker` tests; fix only
  failures caused by T001-T005 in their listed files. Depends on T002-T005.
- [x] T007 Run `pnpm install --frozen-lockfile` if the worktree is not already
  bootstrapped, then run `pnpm run format`, relevant Rust workspace checks, and
  verify no generated or wire contract changed. Depends on T006.
- [x] T008 Cross-check the completed diff against `spec.md`, `plan.md`,
  `tasks.md`, and `.specify/memory/constitution.md`; record the result in
  `analysis.md` and tick every completed task in this file. Depends on T007.

## Phase 4: Review and knowledge

- [x] T009 Run an independent Codex review of the task diff, address confirmed
  significant findings in the affected implementation files, and repeat focused
  verification/review until clean. Depends on T008.
- [x] T010 Distill reusable workspace-session PATH knowledge into the applicable
  project knowledge-base topic, add task id `vk/b2a2-add-vk-cli-tools`, refresh
  the index, and commit the knowledge-base update before handoff. Depends on
  T009.

<!--
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe only within the Phase 2 layer after T001.
- Implementation intentionally needs no API, schema, generated-type, frontend,
  or homelab module task.
-->
