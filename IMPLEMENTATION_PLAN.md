# Implementation plan: Workspaces sidebar loads blank on mobile

Task `vk/b923-workspaces-loadi`. See `SPEC.md` for the diagnosis and design and
`PRIOR_KNOWLEDGE.md` for the constraints inherited from earlier tasks.

## Step 1: backend cache, last-known fallback (`crates/services`)

File: `crates/services/src/services/workspace_diff_stats.rs`

1. Add `last_known: StdMutex<Option<DiffStats>>` to `Slot`. It is a sync
   mutex, readable while a leader holds the async `state` lock.
2. In the leader task, when a result is published (generation still matches),
   also store `outcome.stats` into `last_known`. Also store it when the result
   was invalidated mid-computation: the value is still the newest observation
   of the tree and is only ever used as a display fallback. Failures (`None`)
   leave `last_known` untouched.
3. Add `pub async fn get_within(&self, workspace_id, max_age, deadline,
   compute) -> Option<DiffStats>`:
   - Resolve the slot `Arc` first, so the fallback reads the same slot.
   - `tokio::time::timeout(deadline, self.get_or_compute(...))`.
   - `Ok(result)`: return `result` unchanged. A finished-but-failed compute
     (`None`) stays `None`, as it does today (see the clarification in
     `specs/vk/b923-workspaces-loadi/spec.md`).
   - `Err(Elapsed)`: return `last_known`, cloned. The spawned leader keeps
     running and publishes.
4. `invalidate` is unchanged. It bumps the generation and leaves `last_known`
   alone.
5. Unit tests (paused tokio time):
   - `get_within_returns_last_known_when_recompute_exceeds_deadline`
   - `get_within_returns_none_for_never_computed_slow_workspace_then_serves_published_value`
   - `invalidated_entry_keeps_last_known_fallback_but_is_not_fresh`
   - `get_within_returns_fresh_result_when_compute_finishes_in_time`

## Step 2: backend summaries handler (`crates/server`)

File: `crates/server/src/routes/workspaces/workspace_summary.rs`

1. Add `const SUMMARY_DIFF_STATS_BUDGET: std::time::Duration =
   Duration::from_secs(3)` with a doc comment explaining the request-wide
   budget and the fallback.
2. Before building `diff_futures`, capture
   `let diff_deadline = tokio::time::Instant::now() + SUMMARY_DIFF_STATS_BUDGET;`.
3. Inside each future, compute
   `diff_deadline.saturating_duration_since(Instant::now())` and call
   `WORKSPACE_DIFF_STATS.get_within(...)` instead of `get_or_compute`.
   A zero remaining budget still serves fresh cache hits, because `timeout`
   polls the inner future once before checking the deadline, and otherwise
   serves last-known stats.
4. Add a small pure helper, `remaining_budget(deadline, now)`, with a unit
   test for the saturating behaviour.

## Step 3: frontend fetch deadline and abort (`packages/web-core`)

File: `packages/web-core/src/shared/hooks/useWorkspaces.ts`

1. Export `SUMMARY_REQUEST_TIMEOUT_MS = 20_000`.
2. Add `withRequestTimeout(signal, ms)`, which returns
   `{ signal, cleanup }`. It uses an `AbortController`, a `setTimeout` that
   aborts with a `TimeoutError`-named reason, and forwards an upstream abort.
   It works without `AbortSignal.any`/`timeout` and with fake timers.
3. `fetchWorkspaceSummariesByArchived(archived, hostId, signal?)` passes the
   combined signal to `makeLocalApiRequest`. It keeps the timer armed through
   `response.json()` and calls `cleanup()` in `finally`. A timeout rejects
   with `Workspace summaries request timed out`.
4. Both `useQuery` calls use `queryFn: ({ signal }) => ...(archived, hostId,
   signal)` and `refetchOnWindowFocus: true`.

## Step 4: frontend tests

File: `packages/web-core/src/shared/hooks/useWorkspaces.test.tsx`

1. `aborts a summaries request that never settles and recovers on the next poll`:
   a request mock returns a never-resolving promise that rejects when its
   `signal` aborts. Advance 20 s and check that the signal was aborted and the
   query errored while keeping earlier data. Then resolve normally on the next
   interval and check the new data.
2. `forwards query cancellation to the in-flight summaries request`: start a
   pending request, call `client.cancelQueries`, and check that the captured
   signal is aborted.
3. `refetches summaries when the page becomes visible again`: dispatch
   `visibilitychange` through `focusManager.setFocused(true)` and check that a
   new request was made.
4. The existing tests must still pass.

## Step 5: verify

- `pnpm install --frozen-lockfile` (fresh worktree)
- `cargo test -p services workspace_diff_stats`
- `cargo test -p server workspace_summary`
- `pnpm --filter @vibe/web-core exec vitest run src/shared/hooks/useWorkspaces.test.tsx`
- `pnpm run check`, `pnpm run lint`, `pnpm run format`
- Live evidence: after deploy, `node scripts/time-workspace-summaries.mjs` (or
  curl) should show the p100 latency near DB time + 3 s.

## Step 6: review, knowledge, PR

- Codex review of the diff, iterating until there are no significant findings.
- Update `wiki/coordinator-nfs-load.md` (budget and fallback rule) and
  `docs/knowledge-base/authoritative-snapshot-stream-handoffs.md` (client
  deadline and dedupe trap), plus the indexes.
- Open a PR against `main` and merge it.
