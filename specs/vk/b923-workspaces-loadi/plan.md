# Implementation Plan: Workspaces sidebar metadata loads and recovers on every device

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
- Backend: Rust with axum and tokio (workspace `tokio` has `features = ["full"]`,
  so `tokio::time::timeout` is available). SQLite via sqlx. Worktrees live on
  NFSv3.
- The bulk endpoint is `POST /api/workspaces/summaries`, handled by
  `crates/server/src/routes/workspaces/workspace_summary.rs`
  (`get_workspace_summaries`).
- The diff-stat cache is `crates/services/src/services/workspace_diff_stats.rs`
  (`DiffStatsCache` and the `WORKSPACE_DIFF_STATS` static). It is invalidated
  from `crates/local-deployment/src/container.rs` (two call sites, unchanged).
- Frontend: React 18 with `@tanstack/react-query` ^5.85. The consumer is
  `packages/web-core/src/shared/hooks/useWorkspaces.ts`, which calls
  `makeLocalApiRequest` (a thin `fetch` wrapper in
  `packages/web-core/src/shared/lib/localApiTransport.ts`, so `init.signal` is
  forwarded to `fetch`).
- Tests: tokio `start_paused` unit tests in the cache module, and Vitest with
  jsdom and fake timers in `useWorkspaces.test.tsx`.

## Architecture & Approach

### FR-1, FR-3, FR-4: bounded diff-stat phase with last-known fallback
1. `Slot` gains `last_known: std::sync::Mutex<Option<DiffStats>>`. The leader
   task writes it whenever a compute returns `Some(outcome)`, including when
   an invalidation bumped the generation mid-compute. The value is only a
   display fallback, never "fresh". Failures (`None`) leave it untouched.
2. New `DiffStatsCache::get_within(id, max_age, deadline, compute)`:
   ```text
   slot = slots.entry(id).or_default().clone()
   match timeout(deadline, get_or_compute(id, max_age, compute)):
     Ok(result) -> result                 // fresh hit, in-time compute, or failure (None)
     Err(_)     -> slot.last_known.clone() // still running; the leader keeps going and publishes
   ```
   `get_or_compute` keeps its signature, so there is no churn for other
   callers. The leader is already a `tokio::spawn` that owns the slot lock and
   the semaphore permit, so dropping the waiter at the deadline cancels no git
   work and starts none (FR-4). A pruned slot and a new slot are the same
   thing (`last_known` is `None`).
3. The handler computes one `diff_deadline = Instant::now() +
   SUMMARY_DIFF_STATS_BUDGET` (3 s) before `buffer_unordered(4)`. Each future
   passes `remaining_budget(diff_deadline, Instant::now())` (saturating) to
   `get_within`. Items still queued in the buffer when the budget runs out
   get a zero deadline. `tokio::time::timeout` polls the inner future once
   before it checks the deadline, so an immediate fresh cache hit is still
   served, and otherwise the last-known value is served. The worst-case
   latency of the diff phase is therefore about 3 s plus scheduling.

### FR-2: cheap metadata never waits
This follows from FR-1. The SQL reads (steps 1–7 in the handler) are
unchanged, and the only expensive phase is now bounded.

### FR-5, FR-6, FR-7: client deadline, cancellation, fresh request on the next poll
1. `useWorkspaces.ts` adds `SUMMARY_REQUEST_TIMEOUT_MS = 20_000` and a local
   helper, `withRequestTimeout(upstream: AbortSignal | undefined, ms)`, which
   returns `{ signal, cleanup, timedOut() }`. It uses `AbortController`,
   `setTimeout` and an `abort` listener on the upstream signal. It
   deliberately avoids `AbortSignal.any` and `AbortSignal.timeout`: older iOS
   Safari lacks `any`, and `timeout` ignores Vitest fake timers.
2. `fetchWorkspaceSummariesByArchived(archived, hostId, signal?)` passes
   `signal` to `makeLocalApiRequest`, keeps the timer armed through
   `response.json()`, and calls `cleanup()` in `finally`. If the timer fired,
   it throws `Error('Workspace summaries request timed out')`. It always
   rejects and never returns an empty map, so the retention rule from
   `04618693` applies (FR-6).
3. `useQuery({ queryFn: ({ signal }) => fetch…(archived, hostId, signal) })`.
   Once React Query consumes the signal, it aborts the request when the query
   is cancelled or loses all observers (FR-7). The next `refetchInterval`
   tick then finds no in-flight promise and issues a new request (FR-6).

### FR-8: refresh on return
Both summaries queries set `refetchOnWindowFocus: true`, overriding the global
default of `false` in `packages/web-core/src/shared/lib/queryClient.ts`.
React Query's `focusManager` listens to `visibilitychange`, and `staleTime`
stays at 1 s.

### FR-9
There are no changes to `WorkspaceSummary` or `WorkspaceSummaryResponse`, and
therefore none to `shared/types.ts`.

## Data Model
There are no persisted data changes. `./data-model.md` describes the
in-memory slot state.

## Contracts
The HTTP contract is unchanged. `./contracts/diff-stats-cache.md` documents
the new `get_within` Rust API semantics.

## Research Notes
See `./research.md`. It covers timing evidence, the React Query dedupe
behaviour, rejected alternatives, and the fact that no new dependencies are
needed.

## Constitution Check
- **XLIII (new): bulk enrichment answers within a budget, and polled requests
  carry deadlines.** Directly implemented: a 3 s request-wide budget with a
  last-known/none fallback and no fabricated values, a 20 s client deadline,
  and signal forwarding, with regression tests for both sides.
- **XXXIV: partial projections degrade deterministically.** Rows keep
  rendering from the stream, and missing stats stay `undefined`.
- **XII and the NFS-load rule (`wiki/coordinator-nfs-load.md`).** The work
  keeps its single owner (the spawned leader). Single-flight, the 4-permit
  limit and generation invalidation are unchanged. No background refresher is
  added, since work still starts only from a request.
- **XXXIX: request-scoped reads terminate independently.** The handler's
  latency no longer depends on how long git runs.
- **XL: every awaited client stream settles.** Consistent with it: the
  request/response path now always settles too.
- **II and III: test the contract, small steps.** Paused-time cache tests, a
  hook test with the real QueryClient, and no API shape change.
- **Constraints.** No generated-file edits, no new dependencies, and
  `pnpm run format` runs before completion.
- No deviations.

## Risks & Dependencies
- **Stale stats for one poll.** A workspace whose recompute misses the budget
  shows its previous numbers until the next poll (≤15 s). This is accepted by
  the spec (FR-3).
- **A cold start after a coordinator restart.** Workspaces that have never
  been computed show no stats for the first poll or two while the leaders
  finish. This is better than the current behaviour, where every row is
  blank until the slowest workspace finishes.
- **Test timing.** Cache tests use `tokio::time::pause`. Hook tests must
  advance fake timers past 20 s, and the mock request must respect
  `init.signal`.
- **Deploy dependency.** Live verification (AC-8) needs the coordinator
  redeployed from `main`, which the homelab deploy pipeline does after merge.
