# SPEC: Workspaces sidebar loads blank (no row metadata) on mobile

Task: `vk/b923-workspaces-loadi` ("Workspaces loading Blank", reported with a
phone screenshot of the Workspaces tab at 15:30 UTC on 2026-09-29, deploy
`03e6b4d`).

## Symptom

On the phone (vibe.vasandani.dev through the Cloudflare tunnel) the Workspaces
sidebar rendered its section skeleton and workspace names, but **every row was
blank below its title**: no elapsed time, no diff stats (`files +N -M`), no host
affinity label, no PR badge. Only the pin icon, which comes from the workspace
stream rather than the summaries, was drawn. "Needs Attention" and "Polling"
were empty, and both are driven only by summary fields (`has_unseen_turns`,
`has_pending_approval`, `has_running_poller`).

The same page on desktop, opened minutes later, rendered all of this metadata.

## Diagnosis

Row metadata comes from one request, `POST /api/workspaces/summaries`
(`useWorkspaces.fetchWorkspaceSummariesByArchived`), polled every 15 s by React
Query. Names, pin state and `is_running` come from the separate workspaces
WebSocket stream. So "names but no metadata" means **the stream was fine and
the summaries query never gave the page any data**.

Evidence gathered on the live cluster:

1. **Summaries latency depends on the slowest workspace.** The handler awaits
   diff stats for every active workspace (204 active rows today) before replying.
   Diff stats come from `git status` and diff runs on NFS, at most 4 at a time,
   shared through `WORKSPACE_DIFF_STATS`. Idle entries go stale after 5 minutes.
   Back-to-back probes from a worker measured **0.27 s, 16.6 s, 1.8 s, 0.30 s**
   for the active list and up to 6.8 s for the archived list. That cheap
   metadata (process status, unseen turns, PR, affinity) comes from a handful of
   bulk SQL reads, but it is held hostage by the git work.
2. **Tunnel clients abandon summaries requests.** `cloudflared-connector` on
   think2 logged 142 `Incoming request ended abruptly: context canceled` errors
   for `https://vibe.vasandani.dev/api/workspaces/summaries` since 2026-09-28.
3. **The client has no deadline and one stuck fetch blocks all later polls.**
   `fetchWorkspaceSummariesByArchived` calls `fetch` with no timeout and does
   not forward React Query's `AbortSignal`. React Query dedupes interval and
   focus refetches onto an in-flight promise (`cancelRefetch: false`). So when a
   mobile browser leaves the request hanging (the app is backgrounded or the
   radio changes, and the request never settles), **every later poll joins the
   same dead promise**. The sidebar stays blank until a full reload. On a fresh
   page load there is no earlier snapshot to keep, so the rows are blank from
   the start.
4. `refetchOnWindowFocus: false` also means that coming back to the tab or PWA
   does not ask for fresh summaries.

The WebSocket stream reconnects with backoff and keeps its last snapshot, so it
is not implicated. Whether any workspace was truly running at 15:30 could not
be checked independently (the coordinator DB was not readable from the worker).
The spec treats the empty Running section as consistent with the stream.

## Goals

- G1. A summaries poll **always settles** in bounded time on the client. A hung
  or slow request is aborted and the next poll starts a new request. It never
  joins a dead one.
- G2. The summaries endpoint **responds in bounded time** no matter how slow
  git or NFS is. Cheap metadata must never wait on slow diff stats.
- G3. Bounding latency must **not blank rows that already have stats**. A
  workspace whose fresh diff stats miss the deadline reports its last known
  stats (stale but truthful). If it has never been computed, it reports none.
  The background computation keeps going, so a later poll gets fresh values.
- G4. Returning to the page (visibility or focus) refreshes summaries promptly.
- G5. The existing behaviour stays: a failed refresh keeps the last successful
  snapshot for that host scope (`04618693`). Single-flight and the
  process-wide concurrency bound stay. Invalidation still wins races for
  *fresh* results.

## Non-goals

- Streaming summaries over the WebSocket, or changing the summaries response
  shape or the generated TS types.
- Changing diff-stat freshness tiers, the 14-day idle skip, or git concurrency.
- The WebSocket stream, the sort order or the section categorisation.
- Homelab/Caddy/Cloudflare configuration (out of scope for this repo).

## Design

### Backend: deadline-bounded diff stats with last-known fallback

- `DiffStatsCache` keeps, per slot, the **last published stats**
  (`last_known`), updated whenever a leader publishes a result. It is readable
  without taking the slot's async lock, which a running leader holds for the
  whole computation.
- New `DiffStatsCache::get_within(workspace_id, max_age, deadline, compute)`:
  runs the existing `get_or_compute` path under `tokio::time::timeout(deadline)`.
  - A fresh hit, or a computation that finishes within the deadline, returns
    that result, exactly as today.
  - On timeout it returns `last_known` (`None` if never computed). Because the
    leader is a spawned task that owns the slot lock and the permit, dropping
    the waiting future does not cancel the computation. It still publishes, so
    the next poll is a cache hit. Single-flight and the bound are unchanged.
  - Invalidation bumps the generation but leaves `last_known` in place. It is
    a display fallback only and never counts as fresh.
- `get_workspace_summaries` uses `get_within` with one **request-wide
  deadline** (`SUMMARY_DIFF_STATS_BUDGET`, 3 s), measured from the start of the
  diff phase. That caps the whole diff phase rather than each workspace.
  Workspaces still queued in `buffer_unordered` when the budget runs out go
  straight to their last known stats, so the response time is about
  `DB reads + 3 s` at worst.

### Frontend: deadline, abort and resume refresh

- `fetchWorkspaceSummariesByArchived` accepts React Query's `signal` and
  combines it with a request timeout (`SUMMARY_REQUEST_TIMEOUT_MS`, 20 s)
  through a small helper that works where `AbortSignal.any` or
  `AbortSignal.timeout` is missing (older iOS Safari). On timeout the promise
  rejects. React Query keeps the last successful data (G5), and the next
  interval tick starts a new request (G1).
- Both summaries queries set `refetchOnWindowFocus: true`. React Query's
  focusManager listens to `visibilitychange`, so resuming the PWA refetches
  (G4). `staleTime` stays 1 s.

## Acceptance criteria

- AC1. Backend unit test: when a computation outlasts the deadline,
  `get_within` returns the last known stats, and the computation later
  publishes so a following call is a fresh cache hit with one compute.
- AC2. Backend unit test: a never-computed workspace whose computation
  outlasts the deadline returns `None`, and a later call returns the published
  value without recomputing.
- AC3. Backend unit test: invalidation keeps the last known value as a
  fallback, but a stale fallback never passes as fresh (a within-deadline
  recompute still runs and wins).
- AC4. Frontend test: a summaries fetch that never resolves is aborted after
  the timeout, and the hook recovers with data from the next request.
- AC5. Frontend test: aborting React Query's signal aborts the underlying
  request.
- AC6. `pnpm run check`, `pnpm run lint`, the web-core tests,
  `cargo test -p services` and the summary route tests pass, plus
  `pnpm run format`.

## Risks

- Stale diff stats can show for up to one poll after a slow recompute. This is
  acceptable, since the alternative is showing no metadata at all.
- A 20 s client timeout under a 3 s server budget leaves room for slow mobile
  links and DB contention without false aborts.
