# Prior knowledge: workspace wake-ups and archive lifecycle

Task: vk/e8d6-wake-ups-an-auto. Read-only recall from vibe-kanban/wiki and docs/knowledge-base.

- `wiki/durable-session-wakeups.md`: wake-ups share the durable issue-comment delivery ledger. Pending and dispatching states determine holds; deadlines alone do not. The dispatcher owns active-turn exclusion and launch evidence. Preserve limits, cancellation races, and same-session delivery.
- `wiki/issue-workspace-lifecycle.md`: local SQLite runtime state and remote PostgreSQL issue/workspace state are separate. Remote activation can reopen Done, so a temporary scheduling hold must not blindly send remote unarchive. Remote terminal transitions are transactional. Existing local automatic admission protects work but manual archive intentionally stops persistent processes.
- `docs/knowledge-base/issue-status-side-effects.md`: remote intent is reconciled through a dedicated automatic endpoint, not manual workspace PATCH. Blocked intent remains available for retry.
- `wiki/vk-pollers.md`: pollers are tracked background helpers with serialized poller metadata. They survive turns, require bounded lifetime/completion, and may have durable completion wake-ups.

Source inspection adds: local PR monitoring and merge routes bypass automatic admission; schedule validates the same local Workspace store that PATCH edits; global MCP currently refuses omitted session IDs even when workspace context exists. Exact October 25 provenance is unavailable without historical logs.
