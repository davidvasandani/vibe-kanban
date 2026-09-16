# Prior knowledge: sidebar metadata loss

Task: vk/113f-sidebar-randomly

Searched the Vibe Kanban wiki and docs/knowledge-base for sidebar, metadata, stream, and enrichment. Relevant pages:

- `vibe-kanban/docs/knowledge-base/authoritative-snapshot-stream-handoffs.md`: retain last authoritative data through transport failure; replace on successful snapshots; reset when identity changes.
- `vibe-kanban/wiki/vk-pollers.md`: sidebar poller/activity grouping comes from bulk workspace summaries, not per-row execution subscriptions.
- `vibe-kanban/wiki/workspace-carousel-view.md`: summaries refresh every 15 seconds and drive metadata and grouping across consumers.
- `vibe-kanban/wiki/electric-sync-fallback.md`: errors and recovery must remain distinct; successful fallback restores authority.

Source inspection confirms `useWorkspaces.ts` combines WebSocket identity/pins with HTTP summary metadata. Its summary fetcher converts HTTP, API, and transport errors into successful empty maps, replacing the React Query cache. This explains simultaneous metadata loss while names and pins survive. `keepPreviousData` also spans query-key changes and should not leak summaries between hosts. Preserve same-key query data by rejecting failures and allow successful empty snapshots to clear it.
