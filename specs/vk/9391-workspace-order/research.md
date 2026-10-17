# Research: Stable Workspace Order During Restart

## Existing Data Lifecycle

- `packages/web-core/src/shared/hooks/useWorkspaces.ts` receives active and
  archived workspace records over independent JSON-patch WebSocket streams.
- Summary queries begin only after their corresponding streams initialize and
  are refreshed separately.
- `toSidebarWorkspace` already preserves base `updated_at` as `updatedAt` and
  optionally adds summary `latest_process_completed_at` as
  `latestProcessCompletedAt`.
- `WorkspacesSidebarContainer.tsx` currently sorts “Updated” only by
  `latestProcessCompletedAt`. Missing values are explicitly placed first and
  name-sorted, which explains the post-restart ordering shown in the report.

## Decision

Use the existing base `updatedAt` as the fallback and place records with no
valid selected timestamp last. This fixes startup without changing transport,
query scheduling, persistence, or generated contracts.

## Alternatives Considered

1. **Block rendering until all summaries load.** Rejected because it preserves
   the user's wait and makes the base stream less useful.
2. **Fetch summaries before opening the stream.** Rejected because it couples
   independent data sources, delays initial workspace visibility, and does not
   address summary failures.
3. **Sort only by base `updated_at`.** Rejected because it discards the existing
   more-specific process activity semantics once summaries are available.
4. **Keep missing timestamps first.** Rejected because absent enrichment is not
   evidence of greater recency and causes the reported list takeover.

## Dependencies

No new runtime or development dependencies.
