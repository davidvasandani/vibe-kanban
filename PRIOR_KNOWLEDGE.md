# Prior Knowledge: Stable Workspace Order During Restart

The Vibe Kanban knowledge base is not empty. Searches across
`docs/knowledge-base/` and `wiki/` for workspace sidebar ordering, workspace
summary timestamps, partial-data sorting, and workspace-stream startup found no
page that directly documents this behavior.

## Nearby Knowledge

### `docs/knowledge-base/issue-status-side-effects.md`

- This page confirms that active and archived workspace streams are exposed
  through `WorkspaceContext` and that consumers must tolerate periods when
  workspace stream data is unavailable.
- It does not define sidebar ordering or the relationship between streamed
  workspace records and asynchronously fetched workspace summaries.

## Consequences for This Task

1. Treat startup as a partial-data state: workspace records can be present
   before their summary records.
2. Preserve support for both active and archived streams.
3. Derive the ordering fix from the current code and tests because the project
   knowledge base contains no existing workspace-sort contract to preserve.
4. If the implementation establishes a reusable partial-data ordering rule,
   record it in the knowledge base after the change ships.
