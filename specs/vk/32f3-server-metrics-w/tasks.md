# Tasks: Server Metrics Low-Disk Warnings

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` marks independent work within a layer.

## Phase 1: Contracts and configuration

- [x] T000 Run `/speckit.analyze`, resolve every significant coverage/constitution gap, and update these artifacts before implementation begins (depends on planning completion).
- [ ] T001 Add `DiskAlertThresholds` wire/default validation in `crates/node-metrics/src/types.rs`, load environment overrides with tests in `crates/services/src/services/cluster/metrics.rs`, and expose them on its snapshot (depends on T000).
- [ ] T002 Update metrics patch handling/tests in `crates/server/src/routes/cluster_metrics/patch.rs` and register/regenerate types through `crates/server/src/bin/generate_types.rs`, `shared/types.ts` (depends on T001).
- [ ] T003 [P] Add typed low-disk request/result API models in `crates/api-types/src/issue.rs`, exports, and type generators (depends on none).
- [ ] T004 [P] Add documented threshold options/env/assertions in `homelab/modules/vibe-kanban-rebuild.nix` and its existing Nix tests (depends on none).

## Phase 2: Durable issue resolution

- [ ] T005 Implement transaction-safe low-disk lookup/create and canonical issue body tests in `crates/remote/src/db/issues.rs` and/or a focused DB module (depends on T003).
- [ ] T006 Add authenticated remote low-disk route and route tests in `crates/remote/src/routes/issues.rs` (depends on T005).
- [ ] T007 Add remote client method and a local proxy that re-resolves current server-owned node facts in `crates/services/src/services/remote_client.rs` and `crates/server/src/routes/remote/issues.rs` (depends on T006, T001).

## Phase 3: Alert derivation and presentation

- [ ] T008 [P] Implement pure threshold classification/rollup with exact-boundary and malformed-data tests in `packages/web-core/src/shared/components/ui-new/views/metrics/diskAlerts.ts` and `.test.ts` (depends on T001, T002).
- [ ] T009 Enhance accessible node/filesystem warning presentation in `NodeStrip.tsx`, `DisksPanel.tsx`, and their tests (depends on T008).
- [ ] T010 Add header-owned cache-sharing rollup in `ServerMetricsHeader.tsx` and tests, then mount it through `RightSidebar.tsx` and `RightSidebar.test.tsx` (depends on T008).
- [ ] T011 Wire explicit project context, low-disk API activation, pending/error/reuse navigation, and component tests in `ServerMetricsSectionContainer.tsx`, `.test.tsx`, and `packages/web-core/src/shared/lib/api.ts` (depends on T007, T009).
- [ ] T012 [P] Add English strings and preserve locale fallback behavior in `packages/web-core/src/i18n/locales/en/common.json` (depends on T009, T010, T011).

## Phase 4: Integration and verification

- [ ] T013 Regenerate API types, format the scoped repositories, and run focused Rust, React, generated-type, and Nix tests (depends on T004, T011, T012).
- [ ] T014 Run repository-standard checks feasible in the worker, recording exact results and any environment limitation in `specs/vk/32f3-server-metrics-w/validation.md` (depends on T013).
- [ ] T016 Run independent Codex review, address confirmed findings, repeat to no significant findings, and record in `specs/vk/32f3-server-metrics-w/review.md` (depends on T014).
- [ ] T017 Update and commit the reusable project knowledge base with task tag `vk/32f3-server-metrics-w` in `docs/knowledge-base/` and its `INDEX.md`, or record “no new knowledge to record” (depends on T016).
- [ ] T018 Commit/push the implementation, open a PR against the detected base branch, monitor checks, fix failures, and merge it (depends on T017).

## Dependency layers

- Layer A: T003, T004; T001 begins independently.
- Layer B: T002 after T001; T005 after T003.
- Layer C: T006 after T005; T008 after T001/T002.
- Layer D: T007 after T006; T009 and T010 after T008.
- Layer E: T011 after T007/T009; T012 can land alongside final UI wiring.
- Layer F: T013 → T014 → T016 → T017 → T018.
- Pipeline gate: T000 is the first task and has completed before implementation.
