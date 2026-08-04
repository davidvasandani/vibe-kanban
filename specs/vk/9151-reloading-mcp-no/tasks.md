# Tasks: Reliable MCP Reload

**Plan**: `./plan.md`

## Phase 1: Regression Surface

- [x] T001 Create a feature-local MCP refresh hook with injectable timing/API
  boundaries in
  `packages/web-core/src/features/workspace-chat/model/useMcpRefresh.ts`.
- [x] T002 Add failing lifecycle tests for initial canonical hydration, busy
  reconciliation, polling to terminal state, and stale-session response guards
  in
  `packages/web-core/src/features/workspace-chat/model/useMcpRefresh.test.ts`.

## Phase 2: Integration

- [x] T003 Implement canonical session hydration and selected-session response
  guards in
  `packages/web-core/src/features/workspace-chat/model/useMcpRefresh.ts`
  (depends on T001, T002).
- [x] T004 Implement duplicate-busy reconciliation and canonical pending polling
  in
  `packages/web-core/src/features/workspace-chat/model/useMcpRefresh.ts`
  (depends on T003).
- [x] T005 Wire the hook into the existing toolbar and remove the superseded
  inline state/effects in
  `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`
  (depends on T003, T004).

## Phase 3: Validation and Documentation

- [x] T006 Run the focused Vitest file and `@vibe/web-core` type check; correct
  any failures (depends on T005).
- [x] T007 Run repository formatting and inspect the diff for unrelated changes
  (depends on T006).
- [x] T008 Update
  `docs/knowledge-base/active-mcp-refresh.md` and
  `docs/knowledge-base/INDEX.md` with task `9151-reloading-mcp-no` and the
  canonical client reconciliation rule (depends on T009).
- [x] T009 Run an independent Codex review, address confirmed findings, and
  repeat verification/review until no significant findings remain (depends on
  T006, T007).
- [x] T010 Commit the knowledge-base update after implementation and review are
  complete (depends on T009).

## Dependency order

`T001 → T002 → T003 → T004 → T005 → T006 → T007 → T009 → T008 → T010`
