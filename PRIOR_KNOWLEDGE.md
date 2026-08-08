# Prior Knowledge: Server Affinity Sidebar Polish (`61a3`)

## Knowledge-base matches

### `workspace-affinity-migration.md`

- Affinity has two distinct concepts: requested placement policy and resolved
  worker. UI copy must not conflate an automatic request with an unassigned
  workspace when automatic placement has already resolved to a worker.
- Bulk workspace summaries deliberately carry resolved affinity so list and
  header rendering do not create per-workspace requests.
- Every memo/cache dependency that renders affinity must update after placement
  changes. The collapsed-header server label should therefore use the selected
  workspace summary already consumed by the sidebar.
- Placement controls share scheduler eligibility rules. This task should leave
  those rules and the coordinator-owned migration operation untouched.

### `clustered-workspace-execution.md`

- Persisted workspace affinity is authoritative; it must never be inferred from
  the UI host currently serving the page.
- The coordinator owns placement records and user-facing execution state while
  workers own only their assigned processes. A spacing/header-only change must
  not move placement logic into the client.
- Unreachable or unhealthy workers can make state uncertain. Existing fallback
  labels should remain conservative rather than claiming a healthy assignment
  that summary data does not contain.

## Planning implications

- Use `selectedWorkspaceSummary.serverAffinity` for the collapsed header; do
  not add a query that remains mounted solely to populate closed-section text.
- Resolve the display label from assigned hostname, requested hostname, then
  translated affinity kind, preserving the existing policy/resolution
  distinction.
- Keep the change inside the workspace sidebar and affinity container styling.
- Verify memo dependencies and narrow-width truncation because stale or
  overflowing header content would violate the existing UI-cache convergence
  guidance.

## Baseline note

The checked-out branch predates the merged affinity UI, while `origin/main`
contains it. Implementation planning must first reconcile the branch with the
current Vibe Kanban baseline so the polish lands on the canonical component
instead of recreating affinity behavior.
