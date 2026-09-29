# Tasks: Workspaces sidebar metadata loads and recovers on every device

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Setup
- [x] T001 Install workspace dependencies in the fresh worktree (`pnpm install --frozen-lockfile`). No file changes.

## Phase 2: Core (backend and frontend are independent layers)
- [x] T002 [P] Add `last_known` to `Slot`, publish it from the leader task, and add `DiffStatsCache::get_within` in `crates/services/src/services/workspace_diff_stats.rs` (FR-3, FR-4)
- [x] T003 [P] Add `SUMMARY_REQUEST_TIMEOUT_MS`, the `withRequestTimeout` helper, signal forwarding in `fetchWorkspaceSummariesByArchived`, `queryFn: ({ signal })` and `refetchOnWindowFocus: true` in `packages/web-core/src/shared/hooks/useWorkspaces.ts` (FR-5–FR-8)
- [x] T004 Add `SUMMARY_DIFF_STATS_BUDGET` (3 s) and `remaining_budget`, and switch the diff phase to `get_within` with a request-wide deadline, in `crates/server/src/routes/workspaces/workspace_summary.rs` (FR-1, FR-2; depends on T002)

## Phase 3: Validation
- [x] T005 [P] Cache unit tests (last-known on deadline, never-computed → `None` then published, invalidation keeps the fallback but recomputes, in-time fresh result, failed compute → `None`) in `crates/services/src/services/workspace_diff_stats.rs` (AC-1–AC-3; depends on T002)
- [x] T006 [P] `remaining_budget` saturation unit test in `crates/server/src/routes/workspaces/workspace_summary.rs` (depends on T004)
- [x] T007 [P] Hook tests (a request that never settles is aborted at 20 s and recovers on the next poll, cancellation aborts the request, focus triggers a refetch, existing suite still green) in `packages/web-core/src/shared/hooks/useWorkspaces.test.tsx` (AC-4–AC-7; depends on T003)
- [x] T008 Run `cargo test -p services workspace_diff_stats`, `cargo test -p server workspace_summary`, the web-core Vitest file, `pnpm run check`, `pnpm run lint` and `pnpm run format`. Formatting may touch any changed file. (Depends on T004–T007.)

## Phase 4: Documentation
- [x] T009 [P] Record the budget and fallback rule in `wiki/coordinator-nfs-load.md` and the client deadline and dedupe trap in `docs/knowledge-base/authoritative-snapshot-stream-handoffs.md`, plus `wiki/INDEX.md` and `docs/knowledge-base/INDEX.md` (knowledge-base stage; depends on T008)

<!--
Dependency graph:
T001 → {T002, T003}
T002 → {T004, T005}
T004 → T006
T003 → T007
{T004, T005, T006, T007} → T008 → T009
-->
