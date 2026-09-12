# Tasks: Mobile Deploy Status

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files and may run in parallel within their layer. Each task names the files it changes.

## Phase 1: Deployment metadata contract

- [x] T001 Stamp one UTC build timestamp and reuse it in the immutable release manifest in `local-build.sh`.
- [x] T002 Embed the optional build timestamp and expose it in `UserSystemInfo` in `crates/server/build.rs` and `crates/server/src/routes/config.rs` (depends on T001).
- [x] T003 Regenerate the frontend contract in `shared/types.ts` using `pnpm run generate-types` (depends on T002).

## Phase 2: Frontend state and presentation

- [x] T004 [P] Expose deployment timestamp in user-system state/context in `packages/web-core/src/shared/hooks/useUserSystem.ts` and `packages/web-core/src/shared/hooks/useUserSystemController.ts` (depends on T003).
- [x] T005 [P] Implement compact elapsed-time formatting and deploy-status presentation in `packages/ui/src/components/DeployStatus.tsx` (depends on T003).
- [x] T006 [P] Add deterministic formatter/render/timer tests in `packages/remote-web/src/app/layout/Navbar.test.tsx` (depends on T005).

## Phase 3: Mobile header integration

- [x] T007 Add optional mobile deployment props and responsive indicator placement in `packages/ui/src/components/Navbar.tsx` (depends on T005).
- [x] T008 Pass deployment metadata through `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx` and `packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx` (depends on T004, T007).
- [x] T009 Add mobile navbar integration coverage in `packages/remote-web/src/app/layout/Navbar.test.tsx` (depends on T008).

## Phase 4: Validation and review

- [x] T010 Run locked dependency setup if needed, generated-type checks, targeted Rust/frontend tests, TypeScript checks, lint, and `pnpm run format`; record results in `specs/vk/7596-deploy-status-mo/verification.md` (depends on T006, T009).
- [x] T011 Run independent Codex review, address confirmed findings, repeat verification as needed, and record the clean result in `specs/vk/7596-deploy-status-mo/review.md` (depends on T010).
- [x] T012 Update a relevant topic in `docs/knowledge-base/` or `wiki/`, tag it with `7596-deploy-status-mo`, refresh its index, and commit the knowledge-base change; if nothing reusable emerged, record that decision in `specs/vk/7596-deploy-status-mo/knowledge.md` (depends on T011).
- [x] T013 Merge branch `vk/7596-deploy-status-mo` into its configured base branch after confirming both repositories are clean and only Vibe Kanban scoped files changed (depends on T012).

## Dependency graph

`T001 → T002 → T003 → {T004, T005} → T006/T007 → T008 → T009 → T010 → T011 → T012 → T013`
