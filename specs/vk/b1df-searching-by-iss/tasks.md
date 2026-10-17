# Tasks: Search by Issue ID

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the files it changes.

## Phase 1: Backend contract

- [x] T001 Extend membership-scoped remote global-search SQL with bounded issue
      results matched by `simple_id` in
      `crates/remote/src/routes/global_search.rs`.
- [x] T002 Extend the isolated PostgreSQL fixture and assertions for issue-ID
      case folding, membership isolation, literal matching, and category bounds
      in `scripts/test-global-search-postgres.py` (depends on T001).

## Phase 2: Shared frontend

- [x] T003 [P] Extend the shared search result discriminator and issue-detail
      routing regression in `packages/web-core/src/shared/lib/globalSearch.ts`
      and `packages/web-core/src/shared/lib/globalSearch.test.ts` (depends on
      T001).
- [x] T004 [P] Add the Issues group and rendered selection/display regression in
      `packages/web-core/src/shared/dialogs/global-search/GlobalSearchDialog.tsx`
      and `packages/web-core/src/shared/dialogs/global-search/GlobalSearchDialog.test.tsx`
      (depends on T001).

## Phase 3: Validation and delivery

- [x] T005 Run the focused PostgreSQL, Rust compile/test, and Vitest suites; run
      `pnpm run format`, relevant checks/lint, and `git diff --check`; record
      results in `specs/vk/b1df-searching-by-iss/validation.md` (depends on T002,
      T003, T004).
- [x] T006 Reconcile final behavior into `SPEC.md`, `IMPLEMENTATION_PLAN.md`, and
      `specs/vk/b1df-searching-by-iss/` artifacts (depends on T005).
- [x] T007 Run independent Codex diff review, resolve confirmed significant
      findings, rerun affected verification, and record the clean result in
      `specs/vk/b1df-searching-by-iss/review.md` (depends on T005, T006).
- [x] T008 Update reusable Global Search knowledge in `wiki/global-search.md`,
      refresh `wiki/INDEX.md`, tag `vk/b1df-searching-by-iss`, and commit the
      knowledge base (depends on T007).
- [x] T009 Commit all implementation changes, push the branch, open a pull
      request against the base branch, verify required checks, and merge it
      (depends on T008).

## Dependency graph

`T001 → T002`; after `T001`, `T003` and `T004` are parallel-safe;
`T002 + T003 + T004 → T005 → T006 → T007 → T008 → T009`.
