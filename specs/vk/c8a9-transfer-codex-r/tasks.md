# Tasks: Transfer Codex Rollout Lineage

**Plan**: `./plan.md`

Tasks are dependency-ordered. Tasks marked **[P]** touch independent files and
may run in parallel within their layer.

## Phase 1: Prerequisite and contract fixtures

- [ ] T001 Merge `vk/9a64-vk-workspace-aff` into the task branch, preserve this
  task's `SPEC.md`, `PRIOR_KNOWLEDGE.md`, `IMPLEMENTATION_PLAN.md`, constitution,
  and `specs/vk/c8a9-transfer-codex-r/**`, then confirm a clean prerequisite
  test baseline.
- [ ] T002 [P] Add sanitized pinned-0.144.1 direct/forked/spawned rollout
  fixtures in `crates/executors/tests/fixtures/codex_rollouts/**` containing
  metadata and inert history only.
- [ ] T003 [P] Add the transfer protocol types/error categories and canonical
  manifest digest representation in `crates/cluster-protocol/src/lib.rs`.
- [ ] T004 [P] Add SQL migration for transfer phase/evidence tables in
  `crates/db/migrations/*_codex_session_transfers.sql`.

## Phase 2: Safe local artifact primitives

- [ ] T005 Add module registration/API surface in
  `crates/executors/src/executors/codex.rs` and
  `crates/executors/src/executors/codex/rollout_transfer.rs` (depends T002,
  T003).
- [ ] T006 Implement canonical first-`session_meta` parsing, UUID/path
  validation, ancestor resolution, cycle/conflict detection, and manifest
  hashing/limits in `crates/executors/src/executors/codex/rollout_transfer.rs`.
- [ ] T007 Implement symlink-safe contained opens, regular-file/mutation checks,
  bounded chunk reads, and safe error redaction in
  `crates/executors/src/executors/codex/rollout_transfer.rs`.
- [ ] T008 Implement operation-scoped staging, sequential chunk replay,
  full-file verification, private atomic no-clobber install, identical reuse,
  conflict refusal, full-manifest verification, and scoped cleanup in
  `crates/executors/src/executors/codex/rollout_transfer.rs`.
- [ ] T009 Add exhaustive unit tests for direct/ancestor lineage, malformed and
  cyclic metadata, all limits, traversal/absolute paths, symlink components,
  source mutation, chunk mismatch/order, identical retry, content conflict,
  permissions, readability, partial cleanup, and retention cleanup in
  `crates/executors/src/executors/codex/rollout_transfer.rs` (depends T006–T008).

## Phase 3: Durable coordinator state

- [ ] T010 [P] Add transfer and artifact models plus conditional phase/replay,
  failure, last-needed, and cleanup-candidate queries in
  `crates/db/src/models/codex_session_transfer.rs` and
  `crates/db/src/models/mod.rs` (depends T004).
- [ ] T011 Add database tests for uniqueness, immutable manifest rows,
  conditional transitions, same-operation replay, context conflicts, stale
  recovery, and cleanup protection in
  `crates/db/src/models/codex_session_transfer.rs` (depends T010).

## Phase 4: Worker source and target surface

- [ ] T012 Add the transfer store to `WorkerConfig`/worker startup using the
  executor's `CODEX_HOME` convention in `crates/worker/src/lib.rs`,
  `crates/worker/src/main.rs`, and `crates/worker/src/worker_api.rs` (depends
  T005).
- [ ] T013 Add signed manifest/source-chunk/target-chunk/finalize/verify/abort
  routes with handler-level bounds and complete authority/context validation in
  `crates/worker/src/worker_api.rs` (depends T003, T006–T008, T012).
- [ ] T014 Add exact-router tests for signatures, correlation IDs, workspace,
  operation, source/target worker and thread substitution, nonce replay, body
  caps, chunk retries/conflicts, target verification, and content-free errors in
  `crates/worker/src/worker_api.rs` (depends T013).

## Phase 5: Coordinator transfer orchestration

- [ ] T015 Add bounded typed worker-client methods, chunk response caps, and
  transfer timeout/error mapping in
  `crates/services/src/services/cluster/client.rs` (depends T003, T013).
- [ ] T016 Add local/remote endpoint abstraction and coordinator-mediated
  manifest→chunk→finalize→verify orchestrator in
  `crates/services/src/services/cluster/session_transfer.rs` and
  `crates/services/src/services/cluster/mod.rs` (depends T008, T010, T015).
- [ ] T017 Add orchestrator tests for remote/remote, local/remote, remote/local,
  progress replay, response loss, source mutation, checksum failure, target
  conflict, timeout, abort-partial, and complete verification evidence in
  `crates/services/src/services/cluster/session_transfer.rs` (depends T016).

## Phase 6: Affinity migration lifecycle gate

- [ ] T018 Derive Codex/source/target/thread context exclusively from persisted
  execution/session/placement records and insert/resume transfer after affinity
  operation claim but before stop in
  `crates/server/src/routes/workspaces/affinity.rs` (depends T010, T016).
- [ ] T019 Add the specific pre-stop `SessionTransferFailed` result and safe
  public detail types in `crates/server/src/routes/workspaces/affinity.rs` and
  Rust type-generation declarations in
  `crates/server/src/bin/generate_types.rs` (depends T018).
- [ ] T020 Re-verify durable manifest evidence on crash recovery before
  continuation dispatch while preserving deterministic execution identity in
  `crates/server/src/routes/workspaces/affinity.rs` (depends T018).
- [ ] T021 Add affinity state-machine tests proving missing/corrupt/oversized/
  unauthorized/incomplete transfers leave source and placement unchanged,
  verified transfer precedes stop, all crash windows recover, duplicate
  migration creates one continuation, and non-Codex/same-worker/stopped paths
  are unchanged in `crates/server/src/routes/workspaces/affinity.rs` (depends
  T018–T020).

## Phase 7: Retention and deployment

- [ ] T022 Add bounded startup/periodic cleanup orchestration with active/
  recoverable reference protection in
  `crates/services/src/services/cluster/session_transfer.rs` and deployment
  startup wiring in the existing owner selected during implementation (depends
  T010, T016).
- [ ] T023 [P] Update `homelab/modules/vibe-kanban-rebuild.nix` only if required
  to guarantee identical `CODEX_HOME`, service ownership, or private directory
  semantics; add/evaluate the module's existing tests. If no change is needed,
  record that verification in `specs/vk/c8a9-transfer-codex-r/validation.md`.
- [ ] T024 Add cleanup tests for 24-hour partial and 30-day verified retention,
  active/recoverable protection, symlink/type substitution, bounded passes,
  idempotent repeats, and unreachable-node retention in the files touched by
  T009/T011/T017/T022 (depends T022).

## Phase 8: End-to-end compatibility and verification

- [ ] T025 Add a two-worker temporary-`CODEX_HOME` migration integration test
  that proves ancestor files reach the target and pinned Codex `thread/fork`
  resolves the migrated leaf in the appropriate cluster/server integration test
  module (depends T021).
- [ ] T026 [P] Regenerate `shared/types.ts` with
  `pnpm run generate-types` and add/update frontend affinity outcome rendering
  only if the new public outcome requires it in
  `packages/web-core/src/pages/workspaces/ServerAffinitySectionContainer.tsx`
  and its tests (depends T019).
- [ ] T027 Run `pnpm install --frozen-lockfile`, `pnpm run format`, focused
  executor/db/worker/services/server tests, `pnpm run backend:check`, relevant
  clippy/type generation checks, the two-worker test, and Nix evaluation; record
  exact results and any environment-only limitations in
  `specs/vk/c8a9-transfer-codex-r/validation.md` (depends T024–T026).

## Phase 9: Review and knowledge

- [x] T028 Run an independent Codex review of the complete diff, address every
  confirmed significant finding, rerun impacted verification, and repeat until
  clean; record it in `specs/vk/c8a9-transfer-codex-r/review.md` (depends T027).
- [x] T029 Distill reusable rollout-transfer and pre-lifecycle evidence lessons
  into `docs/knowledge-base/codex-rollout-transfer.md`, refresh
  `docs/knowledge-base/INDEX.md`, tag with `c8a9-transfer-codex-r`, and commit the
  knowledge-base update (depends T028).

## Dependency layers

- Layer A: T001.
- Layer B parallel: T002, T003, T004.
- Layer C: T005–T009; T010–T011 may proceed once T004 lands.
- Layer D: T012–T014 and T015–T017 in dependency order.
- Layer E: T018–T021.
- Layer F: T022; T023 may run in parallel after the runtime path is known; T024.
- Layer G: T025 and T026 parallel, then T027.
- Layer H: T028, then T029.
