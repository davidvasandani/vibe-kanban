# Tasks: Single-Value Browser Titles

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` marks work that is safe to perform together
within the same dependency layer because it touches independent files.

## Phase 1: Regression Contract

- [x] T001 Install the repository's locked frontend dependencies with
  `pnpm install --frozen-lockfile` (no authored file changes expected).
- [x] T002 Add focused jsdom hook coverage for a single specific title, ordered
  fallback selection, whitespace-only candidates, all-absent fallback, and
  rerender updates in
  `packages/web-core/src/shared/hooks/usePageTitle.test.tsx` (depends on T001).
- [x] T003 Run the focused hook test before implementation and confirm its
  single-value assertions fail against the current concatenating behavior
  (depends on T002; no file changes).

## Phase 2: Implementation

- [x] T004 Update
  `packages/web-core/src/shared/hooks/usePageTitle.ts` to select the first
  meaningful candidate, trim its surrounding whitespace, and use `Vibe Kanban` only
  when none exists (depends on T003).
- [x] T005 Verify the ordered issue-title/project-name fallback remains explicit
  and readable in
  `packages/web-core/src/pages/kanban/ProjectKanban.tsx`; change it only if the
  hook call needs clarification (depends on T004).
- [x] T006 Run the focused hook test and confirm all browser-title behavior
  passes after T004–T005 (depends on T005; no file changes).

## Phase 3: Validation

- [x] T007 [P] Run `pnpm --filter @vibe/web-core run check` (depends on T006; no
  file changes).
- [x] T008 [P] Run the relevant web-core test suite (depends on T006; no file
  changes).
- [x] T009 Run repository `pnpm run format`, `git diff --check`, and scoped diff
  inspection (depends on T007, T008; formatting may touch only authored files).

## Phase 4: Review and Delivery

- [x] T010 Run an independent Codex review of the task diff, fix every confirmed
  significant finding, and repeat relevant validation/review until clean
  (depends on T009).
- [x] T011 Distill the reusable single-value title contract into the project
  wiki, tag it with `vk/8c71-don-t-concatenat`, refresh `wiki/INDEX.md`, and
  commit the knowledge base (depends on T010).
- [x] T012 Commit remaining task changes, push the branch, open a pull request
  against the base branch, satisfy merge requirements, and merge it (depends on
  T011).

## Dependency Graph

`T001 → T002 → T003 → T004 → T005 → T006 → {T007, T008} → T009 → T010 → T011 → T012`
