# Tasks: Mobile Deploy Status

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` marks tasks safe to execute together within
their dependency layer because they touch independent files.

## Phase 1: Server contract

- [x] T001 Add one release build timestamp to `local-build.sh`, embed it through
      `crates/server/build.rs`, and expose additive optional
      `UserSystemInfo.deployment_timestamp` in
      `crates/server/src/routes/config.rs`.
- [x] T002 Add focused coverage for timestamp formatting, interval updates, and
      safe missing-metadata behavior in
      `packages/remote-web/src/app/layout/Navbar.test.tsx` (depends on T001).
- [x] T003 Regenerate `shared/types.ts` with `pnpm run generate-types`
      (depends on T001).

## Phase 2: Frontend state and presentation

- [x] T004 Extend the user-system state/controller with deployment start time in
      `packages/web-core/src/shared/hooks/useUserSystem.ts` and
      `packages/web-core/src/shared/hooks/useUserSystemController.ts`
      (depends on T003).
- [x] T005 [P] Implement compact deployment-age formatting and its focused tests
      in `packages/ui/src/components/MobileDeployStatus.tsx` and the nearest
      configured test location (depends on T003).
- [x] T006 Add optional deployment metadata presentation to the mobile branch of
      `packages/ui/src/components/Navbar.tsx` while preserving desktop behavior
      (depends on T005).
- [x] T007 Thread `appVersion` and deployment start time through
      `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`
      into `Navbar` (depends on T004 and T006).
- [x] T008 Confirm `SharedAppLayout.tsx` continues using the existing desktop
      `AppBar` props and mobile `NavbarContainer` composition without changing
      update/refresh behavior (depends on T007).

## Phase 3: Validation

- [x] T009 [P] Run focused frontend tests and `@vibe/ui` / `@vibe/web-core`
      type checks (depends on T007).
- [x] T010 [P] Run focused Rust tests plus generated-type verification
      (depends on T002 and T003).
- [x] T011 Perform narrow mobile layout verification using available render or
      browser tooling; confirm status plus all existing header controls fit and
      no page-level horizontal overflow occurs (depends on T007).
- [x] T012 Run repository-required `pnpm run lint` and `pnpm run format`
      (depends on T009 and T010).
- [x] T013 Run `git diff --check` and inspect the complete diff against the spec,
      constitution, and generated-file constraints (depends on T011 and T012).

## Phase 4: Review and handoff

- [x] T014 Run the independent Codex diff review, address confirmed findings,
      and repeat validation/review until no significant findings remain
      (depends on T013).
- [x] T015 Distill reusable shipped knowledge into
      `docs/knowledge-base/` and update `docs/knowledge-base/INDEX.md`, or record
      that no new knowledge emerged; commit the knowledge-base update separately
      (depends on T014).
- [x] T016 Commit the completed feature and merge branch
      `vk/6e4c-deploy-status-mo` into the base branch (depends on T015).

## Parallel notes

T005 is independent of T004 after the generated contract exists. T009 and T010
exercise separate frontend and backend lanes. All formatting, final diff review,
independent review, documentation, and merge tasks remain serial after those
lanes converge.

## Implementation results

- `pnpm --filter @vibe/web-core test -- MobileDeployStatus.test.tsx`: 26 files,
  239 tests passed.
- `@vibe/ui` and `@vibe/web-core` TypeScript checks passed.
- Focused server timestamp test passed.
- `pnpm run generate-types:check`, `pnpm run format`, and `pnpm run lint`
  passed, including both Rust workspaces and unused-i18n checks.
- Independent `codex review --uncommitted` reported no discrete correctness
  issue or significant finding.
- Authored files pass `git diff --check`; generator-owned `shared/types.ts`
  retains its established trailing-space output and passes the generator check.
- Reusable knowledge was committed as `69749fbf` in
  `docs/knowledge-base/responsive-deployment-identity.md` and the index.
- During merge, current `main` already contained the independently completed
  `vk/7596-deploy-status-mo` implementation. The merge retained that tested,
  release-stamped implementation and discarded duplicate component/plumbing
  from this branch while preserving this task's SpecKit and knowledge artifacts.
