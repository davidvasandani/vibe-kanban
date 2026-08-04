# Tasks: Coordinator Workspace Placement

**Plan**: `./plan.md`

## Phase 1: Contract and intent

- [ ] T001 Add backward-compatible `run_on_coordinator` request intent in `crates/db/src/models/requests.rs`.
- [ ] T002 Add and unit-test the closed placement-intent resolver in `crates/server/src/routes/workspaces/create.rs` (depends on T001).
- [ ] T003 Integrate resolved placement intent into clustered workspace creation, validating before workspace mutation and retaining local placement for coordinator intent in `crates/server/src/routes/workspaces/create.rs` (depends on T002).

## Phase 2: Shared UI

- [ ] T004 [P] Add a pure placement-selection serializer and its unit tests in `packages/web-core/src/shared/lib/workspacePlacement.ts` and `packages/web-core/src/shared/lib/workspacePlacement.test.ts` (depends on T001).
- [ ] T005 Add **Coordinator** to the existing selector and submit the serialized placement fields in `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx` (depends on T004).
- [ ] T006 [P] Add the coordinator localization string in the existing locale resource file(s) under `packages/web-core/src/i18n/locales/` (depends on T005 path discovery; independent of T002/T003).

## Phase 3: Generated contract and verification

- [ ] T007 Regenerate `shared/types.ts` with `pnpm run generate-types` (depends on T001).
- [ ] T008 [P] Run focused Rust tests for workspace creation intent and placement behavior (depends on T002, T003).
- [ ] T009 [P] Run focused web-core tests and type checks (depends on T004, T005, T006, T007).
- [ ] T010 Run repository formatting, generated-type verification, lint, and broader checks required by the touched surfaces (depends on T007-T009).

## Phase 4: Review and knowledge

- [ ] T011 Run independent Codex diff review, address confirmed findings, and repeat relevant verification until no significant findings remain (depends on T010).
- [ ] T012 Distill reusable placement-intent knowledge into `docs/knowledge-base/clustered-workspace-execution.md`, tag it `2fe7-vk-coordinator-m`, refresh `docs/knowledge-base/INDEX.md`, and commit the knowledge-base update (depends on T011).
