# Prior knowledge: Workspaces sidebar loading blank

Task: `vk/b923-workspaces-loadi`. This file pulls together what the project
knowledge bases (`wiki/` and `docs/knowledge-base/`) already record about this
problem area. The knowledge bases were only read, not changed, in this stage.

## Relevant pages

- `wiki/coordinator-nfs-load.md` (task `vk/78a5-analyze-and-redu`)
- `docs/knowledge-base/authoritative-snapshot-stream-handoffs.md`
  (summary retention on failed refresh, `vk/113f-sidebar-randomly`)
- `docs/knowledge-base/workspace-summary-ordering.md` (`vk/9391-workspace-order`)
- `docs/analysis/coordinator-nfs-io-pressure.md` (numbers behind the NFS page)

## What we already know

1. **Two independent sources feed the sidebar.** Names, pins and `is_running`
   come from the JSON-patch WebSocket streams. PR, diff, approval, poller,
   unseen-activity, elapsed-time and affinity metadata come from
   `POST /api/workspaces/summaries`, polled every 15 s for both the active
   and the archived scope. "Names but no enrichment" points to the summaries
   query, not to deleted workspaces. That is exactly what the screenshot
   shows.
2. **A failed refresh must reject, never resolve to an empty map.** React
   Query then keeps the last successful snapshot for the same key. Host and
   archive scope stay in the query key. Do not use `keepPreviousData`, because
   it leaks across hosts. Commit `04618693` shipped this. Any new
   timeout or abort path must therefore **reject**, so this protection covers
   it too.
3. **Summaries cost is NFS-bound and scales with open clients.** Each
   workspace's diff stats run several git subprocesses over NFS. Before the
   shared cache, one request took 34–43 s. The fix was
   `WORKSPACE_DIFF_STATS`, a single-flight cache per workspace, with a
   process-wide limit of 4 permits, tiered staleness (30 s running, 5 min
   idle, 60 min archived) and generation-based invalidation.
4. **The leader runs in its own `tokio::spawn` and owns the slot lock and the
   permit.** A dropped or cancelled waiter therefore does not cancel the git
   work or break the concurrency limit. This is what makes a waiter-side
   deadline safe: the work finishes and is published for the next poll.
5. **Rejected alternatives on record:** NFS mount tuning (breaks coherency), a
   background refresher (does work when nobody is looking) and client-side
   throttling alone. A request-driven deadline with a last-known fallback is
   none of these. Work still only starts because a client asked.
6. **Ordering must tolerate summaries that have not arrived.** The sort falls
   back to the streamed `updatedAt` when `latestProcessCompletedAt` is
   missing. A missing summary is a normal state and does not prove the
   workspace is inactive.
7. Tools: `node scripts/time-workspace-summaries.mjs` times the endpoint. The
   service account can `ssh think2` from any worker, which gives access to the
   `cloudflared-connector` and `vibe-kanban-dev` journals.

## Gaps this task fills

- Nothing bounded how long **one summaries request** could take. Every row's
  metadata waited on the slowest `git status`. Measured today: 0.27–16.6 s
  from a worker.
- Nothing bounded how long the **client** waits, and React Query dedupes polls
  onto an in-flight promise. So one request that never settles on mobile
  blanks the sidebar until a reload. The tunnel logged 142 abandoned
  summaries requests since 2026-09-28.
