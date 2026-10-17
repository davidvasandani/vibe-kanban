# Tasks: Preserve Preview App Navigation URLs

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the files it changes.

## Phase 1: Persisted contract

- [x] T001 Add optional `current_route` to `PreviewSettingsData` and serde
      compatibility tests in `crates/db/src/models/scratch.rs`.
- [x] T002 Regenerate the TypeScript contract in `shared/types.ts` using
      `pnpm run generate-types` (depends on T001).

## Phase 2: Shared frontend behavior

- [x] T003 Extend the complete-write and clear semantics, plus expose durable
      current-route persistence, in
      `packages/web-core/src/shared/hooks/usePreviewSettings.ts`,
      `packages/web-core/src/shared/hooks/useScratch.ts`,
      `packages/web-core/src/shared/hooks/useLocalStorageScratch.ts`, and
      `packages/web-core/src/shared/lib/api.ts` (depends on T002).
- [x] T004 Extract/test canonical route cleanup and rebasing helpers in
      `packages/web-core/src/pages/workspaces/previewUrlState.ts` and
      `packages/web-core/src/pages/workspaces/previewUrlState.test.ts` (depends on
      T002).
- [x] T005 Wire retained-route restoration, accepted-navigation persistence, and
      fragment-preserving proxy iframe construction in
      `packages/web-core/src/pages/workspaces/PreviewBrowserContainer.tsx`
      (depends on T003, T004).

## Phase 3: Validation and documentation

- [x] T006 Run focused Vitest and Rust tests, generated-type verification,
      frontend/type checks, formatting, and linting; update implementation files
      only as required to resolve findings (depends on T005).
- [x] T007 [P] Reconcile final behavior into `SPEC.md`,
      `IMPLEMENTATION_PLAN.md`, and `specs/vk/c89d-address-fable-fo/*.md`
      (depends on T005).
- [x] T008 Independently review the complete diff and address all confirmed
      significant findings in the files named by this tasks list (depends on
      T006, T007).
- [x] T009 Record reusable preview URL persistence invariants in `wiki/`, refresh
      `wiki/INDEX.md`, and commit the knowledge-base update (depends on T008).
- [x] T010 Commit all task changes, open a pull request against `main`, verify its
      checks, and merge it (depends on T009).
