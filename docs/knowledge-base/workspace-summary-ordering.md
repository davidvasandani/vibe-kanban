# Ordering workspace projections while summaries load

Tags: `vk/9391-workspace-order`

## Base records and summaries arrive independently

The workspace sidebar is a projection of two sources in
`packages/web-core/src/shared/hooks/useWorkspaces.ts`:

- active and archived `WorkspaceWithStatus` records arrive over JSON-patch
  WebSocket streams; and
- `WorkspaceSummary` maps are fetched only after each stream initializes and
  refresh independently afterward.

Consequently, a workspace can be visible before its summary exists. This is a
normal startup/reconnect state, not evidence that the workspace has no activity.
Projection logic must remain useful with the base record alone.

## Updated-time ordering contract

For the sidebar's “Updated” sort, prefer the valid
`latestProcessCompletedAt` summary value and fall back to the streamed
workspace's persisted `updatedAt`. The summary retains the product's precise
completed-process semantics, while the base value prevents every summary-less
workspace from falling into an alphabetical startup bucket.

Do not place absent values first. Missing enrichment is not recency evidence.
If neither candidate is a valid date, place the workspace after timestamped
records in both ascending and descending modes.

## Determinism and shared application

Ordering is resolved in this sequence:

1. pinned workspaces before unpinned workspaces;
2. valid selected timestamps before missing/invalid timestamps;
3. chosen ascending or descending timestamp order;
4. workspace name; and
5. unique workspace ID.

The final identity tie-breaker prevents input/patch insertion order from
changing the rendered list. Apply one comparator after filtering to both active
and archived lists, before pagination, so the two streams cannot drift into
different behavior.

## Testing partial projections

Keep pure comparator coverage for both lifecycle points:

- base-only startup data, proving `updatedAt` beats alphabetical order; and
- enriched data, proving a valid process-completion timestamp takes precedence.

Also cover invalid enrichment fallback, missing values in both directions,
pinning, stable ties, and input immutability. This is more durable than a full
sidebar render because the contract is data ordering rather than DOM layout.
