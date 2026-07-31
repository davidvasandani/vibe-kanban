# Tasks: Clustered Vibe Kanban

**Plan**: `./plan.md`

Tasks are dependency-ordered. `[P]` tasks in the same layer touch independent
files and may be completed together.

## Layer 1 — Protocol and persistence foundations

- [x] T001 Create versioned worker protocol types and serialization tests in
  `crates/cluster-protocol/Cargo.toml`, `crates/cluster-protocol/src/lib.rs`,
  and root `Cargo.toml`.
- [x] T002 [P] Add worker-node, placement, execution-job, and repository-lock
  migrations in `crates/db/migrations/`.
- [x] T003 Add DB models, transition methods, and migration tests in
  `crates/db/src/models/{worker_node,workspace,execution_process,execution_worker_job,repository_admin_lock}.rs`
  and `crates/db/src/models/mod.rs`. Depends on T002.
- [x] T004 [P] Add cluster configuration parsing/defaults in
  `crates/services/src/services/cluster/config.rs` and module exports. Depends
  on T001.
- [x] T005 Register new Rust API types for generation in
  `crates/server/src/bin/generate_types.rs`. Depends on T003.

## Layer 2 — Worker visibility and scheduling

- [x] T006 Implement worker registry, heartbeat expiry, draining, mount
  challenge validation, and unit tests in
  `crates/services/src/services/cluster/registry.rs`. Depends on T003, T004.
- [x] T007 [P] Implement eligibility, manual selection validation, weighted
  scoring, deterministic tie breaking, and tests in
  `crates/services/src/services/cluster/scheduler.rs`. Depends on T003, T004.
- [x] T008 Implement authenticated coordinator worker endpoints in
  `crates/server/src/routes/workers.rs`, route wiring in
  `crates/server/src/routes/mod.rs`, and deployment state wiring in
  `crates/local-deployment/src/lib.rs`. Depends on T006, T007.
- [x] T009 [P] Add worker administration API client methods in
  `packages/web-core/src/shared/lib/api.ts`. Depends on T005, T008.
- [x] T010 Add worker health/draining administration UI and tests under
  `packages/web-core/src/shared/dialogs/settings/` and localized strings under
  `packages/web-core/src/i18n/locales/`. Depends on T009.

## Layer 3 — Worker daemon and mount safety

- [x] T011 Create worker crate/binary lifecycle in
  `crates/worker/Cargo.toml`, `crates/worker/src/{lib,main}.rs`, root
  `Cargo.toml`, and `local-build.sh`. Depends on T001, T004.
- [x] T012 Implement shared-root canonical path authorization and symlink-escape
  tests in `crates/worker/src/path_authority.rs`. Depends on T011.
- [x] T013 [P] Implement mount-table/export, coordinator-probe, writability,
  filesystem identity, and UID/GID validation with fixtures in
  `crates/worker/src/mount_health.rs`. Depends on T011.
- [x] T014 Implement authenticated registration/heartbeat and compatibility
  negotiation in `crates/worker/src/server.rs`. Depends on T006, T011, T013.
- [x] T015 Add bounded event journal, cursor replay/gap behavior, atomic terminal
  evidence, and tests in `crates/worker/src/journal.rs`. Depends on T011.

## Layer 4 — Shared provisioning and Git ownership

- [x] T016 Add canonical shared workspace/repository/log path resolution in
  `crates/workspace-manager/src/workspace_manager.rs`. Depends on T003, T004.
- [x] T017 Implement repository-scoped in-process locking plus SQLite fencing
  in `crates/worktree-manager/src/worktree_manager.rs`. Depends on T003.
- [x] T018 Remove/serialize unsafe repo-wide prune and cover concurrent
  create/remove/prune in `crates/worktree-manager/src/worktree_manager.rs`.
  Depends on T017.
- [x] T019 Change workspace creation to reserve placement before provisioning
  and persist `ready`/`failed` truthfully in
  `crates/local-deployment/src/container.rs`,
  `crates/services/src/services/container.rs`, and workspace creation routes.
  Depends on T007, T016, T018.
- [x] T020 [P] Add manual placement create controls and workspace affinity/state
  display under `packages/web-core/src/`. Depends on T005, T019.

## Layer 5 — Remote coding-agent execution

- [x] T021 Implement idempotent worker job supervision, request-digest conflict,
  process groups, scoped environment delivery, and executor event adaptation in
  `crates/worker/src/execution.rs`. Depends on T012, T015.
- [x] T022 [P] Implement graceful/TERM/KILL cancellation state machine and
  child/grandchild fixture tests in `crates/worker/src/cancellation.rs`. Depends
  on T021.
- [x] T023 Add coordinator worker client, retry/idempotency, event
  acknowledgement, and replay-gap handling in
  `crates/services/src/services/cluster/client.rs`. Depends on T008, T014, T015.
- [x] T024 Refactor local process execution behind a dispatcher boundary and add
  remote dispatch while preserving local behavior in
  `crates/services/src/services/container.rs` and
  `crates/local-deployment/src/container.rs`. Depends on T019, T021, T023.
- [x] T025 Feed worker events into existing `MsgStore`, normalization,
  persistence, and WebSocket paths with ordering tests in
  `crates/local-deployment/src/container.rs`. Depends on T024.
- [x] T026 Implement remote cancellation route semantics and explicit
  indeterminate state in `crates/server/src/routes/execution_processes.rs` and
  related DB/API types. Depends on T022, T024.

## Layer 6 — Reconciliation and destructive safety

- [x] T027 Implement coordinator reconnect/job-inventory reconciliation,
  unknown-job quarantine, terminal evidence conflict handling, and tests in
  `crates/services/src/services/cluster/reconcile.rs`. Depends on T023, T026.
- [x] T028 Implement worker restart job adoption policy and retained terminal
  inventory in `crates/worker/src/recovery.rs`. Depends on T021, T022.
- [x] T029 Reorder startup so reconciliation precedes execution/workspace
  cleanup in `crates/local-deployment/src/lib.rs`. Depends on T027.
- [ ] T030 Make expiry and orphan cleanup affinity-aware and retain on active,
  unreachable, stale, or conflicting worker evidence in
  `crates/local-deployment/src/` and
  `crates/workspace-manager/src/workspace_manager.rs`. Depends on T027, T029.
- [ ] T031 Add disconnect/reconnect, missing-job, returning-worker, cursor-gap,
  and cleanup-race integration tests under `crates/worker/tests/` and
  `crates/local-deployment/`. Depends on T025, T028, T030.

## Layer 7 — Full workspace interaction

- [ ] T032 Route setup, cleanup, archive, reviews, follow-ups, dev servers, and
  background helpers through persisted affinity in
  `crates/local-deployment/src/container.rs`. Depends on T024, T027.
- [ ] T033 Implement correlated remote approvals/questions and disconnect
  policies in `crates/worker/src/interaction.rs` and coordinator executor
  integration. Depends on T025.
- [ ] T034 Add local/remote terminal abstraction and bidirectional authenticated
  proxying in `crates/services`, `crates/worker/src/terminal.rs`, and
  `crates/server/src/routes/terminal.rs`. Depends on T012, T023, T024.
- [ ] T035 Add affinity/job/generation-aware preview routing in
  `crates/preview-proxy`, `crates/worker/src/preview.rs`, and
  `crates/server/src/routes/preview.rs`. Depends on T023, T032.
- [ ] T036 [P] Resolve relay/editor navigation from persisted affinity in
  relevant `crates/relay-hosts`, server routes, and `packages/web-core/src`.
  Depends on T020.
- [ ] T037 Add frontend interrupted/indeterminate/output-incomplete states and
  terminal/preview routing tests in `packages/web-core/src`. Depends on T026,
  T034, T035.

## Layer 8 — Deployment

- [ ] T038 Add coordinator/worker role, shared NFS root, credential file,
  listener, scheduling, and health options in
  `../homelab/modules/vibe-kanban-rebuild.nix`. Depends on T011.
- [ ] T039 Add identical NFS mount ordering, consistent service UID/GID,
  coordinator-local data path, systemd credentials, worker units, LAN firewall,
  and capacity/mount health in `../homelab/modules/vibe-kanban-rebuild.nix`.
  Depends on T014, T038.
- [ ] T040 Add Nix evaluation assertions/tests for invalid role, missing
  credential, mismatched/local shared root, service ordering, and firewall in
  the closest Vibe Kanban module test files. Depends on T039.
- [ ] T041 Document two-node Slice 2 rollout, drain, credential rotation,
  snapshot/recovery, mount-loss, reconciliation, and rollback in Vibe
  Kanban-scoped docs. Depends on T031, T039.

## Layer 9 — Generated artifacts and verification

- [ ] T042 Regenerate `shared/types.ts` and schemas with
  `pnpm run generate-types`; do not hand-edit generated outputs. Depends on
  T005, T026, T037.
- [ ] T043 Install dependencies with `pnpm install --frozen-lockfile` and run
  focused protocol, DB, scheduler, worker, container, frontend, and Nix tests.
  Depends on T031, T037, T040, T042.
- [ ] T044 [P] Run Rust workspace tests/checks and generated-type checks.
  Depends on T043.
- [ ] T045 [P] Run frontend type checks, rendered tests, and lint. Depends on
  T043.
- [ ] T046 Run `pnpm run format`, re-run affected validation, and inspect both
  repository diffs for unrelated changes. Depends on T044, T045.
- [ ] T047 Perform the two-disposable-node Slice 2 validation where deployment
  access is available and record evidence. Depends on T046.

## Layer 10 — Review and knowledge

- [ ] T048 Run independent Codex review across the Vibe Kanban diff and scoped
  homelab module diff, address confirmed findings, and repeat verification/review
  until no significant findings remain. Depends on T046 and T047 when
  available.
- [ ] T049 Update `docs/knowledge-base/` and its index with reusable clustered
  execution, shared-mount validation, replay/reconciliation, and shared-Git
  safety knowledge tagged `957e-clustered-vibe-k`; commit the knowledge-base
  update. Depends on T048.

## Parallel execution notes

- T002 and T004 are independent after the protocol shape is fixed.
- T006 and T007 share only DB/config contracts and can be implemented
  independently.
- T012, T013, and T015 touch independent worker modules.
- T020 can proceed alongside worker execution after the workspace API shape is
  stable.
- T022 and T023 are independent worker/coordinator implementations.
- T036 is independent of terminal/preview implementation once affinity is
  exposed.
- T044 and T045 are independent read-only verification layers.
