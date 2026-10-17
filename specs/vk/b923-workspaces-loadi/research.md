# Research: Workspaces sidebar loads blank

## Evidence (2026-09-29, live cluster)
- **Screenshot (15:30 UTC, deploy `03e6b4d`).** Rows show a title and a pin
  only: no elapsed time, stats, host or PR. Needs Attention and Polling are
  empty. Every missing field comes from `WorkspaceSummary`. Title, pin and
  `is_running` come from the stream.
- **The same UI on desktop via the managed browser a few minutes later**
  showed all of the metadata (for example "7h ago, 267 +27012 -1545, think1").
- **Summaries latency from think4 to the coordinator on :3334:** 0.27 s,
  **16.6 s**, 1.76 s, 0.30 s, 0.27 s for the active list, and up to 6.8 s for
  the archived list. There are 204 active rows.
- **`cloudflared-connector` on think2:** 142 `Incoming request ended abruptly:
  context canceled` errors for `/api/workspaces/summaries` since 2026-09-28.
  Clients abandon these requests often.

## Decision: request-wide server budget, 3 s
- The budget is per request, not per item. With `buffer_unordered(4)`, a
  per-item timeout would still add up to about N/4 × timeout.
- 3 s is about 10× the warm median, and it caps the current worst case of
  16.6 s. It is short enough that a phone on a poor link still gets a
  response well inside any proxy timeout.

## Decision: last-known fallback only for unfinished computations
- A failed computation (the worktree is missing or git errored) keeps
  today's behaviour of reporting none. Old numbers for a broken worktree
  would mislead (see spec clarifications).
- `last_known` survives invalidation. An invalidated value is still the newest
  observation, and it is only used when a fresh value is not ready in time.

## Decision: client deadline 20 s, signal forwarded
- React Query v5 `Query.fetch`: when a fetch is already in flight and
  `cancelRefetch` is not set, the caller receives the **existing promise**.
  `refetchInterval` ticks and focus refetches use `cancelRefetch: false`, so
  a request that never settles captures every later poll. That is the root
  cause of "blank until reload".
- React Query aborts the `signal` passed to `queryFn` only when the query is
  cancelled or unobserved. It never aborts because of elapsed time, so a
  deadline has to come from the client side.
- The repo already uses `AbortSignal.any` and `AbortSignal.timeout` in
  `globalSearch.ts`. Here a manual `AbortController` and `setTimeout` is used
  instead:
  - `AbortSignal.any` needs Safari 17.4 or later, and the reporter is on iOS;
  - `AbortSignal.timeout` does not respect Vitest fake timers, which is why
    `globalSearch.test.ts` has to mock it.
- 20 s leaves plenty of margin above a response of about 3 s plus DB time,
  plus a slow mobile link.

## Decision: `refetchOnWindowFocus: true` for summaries only
- The global default stays `false` to avoid refetch storms across every
  query. Summaries are cheap to re-request now that they are bounded and
  cached.

## Alternatives rejected
- **Stale-while-revalidate inside `get_or_compute`** (always return the stale
  entry immediately and recompute in the background). It changes behaviour
  for every caller and weakens freshness where time is available. The
  deadline approach serves fresh data whenever it can.
- **Splitting diff stats into a second endpoint.** It changes the API and
  the generated types, and it doubles the polling. It is not needed once the
  endpoint is bounded.
- **A background refresher.** Already rejected in
  `wiki/coordinator-nfs-load.md`.
- **Only a client timeout.** The phone would recover, but every client would
  still wait up to the slowest git status for any metadata.

## Dependencies
None added.
