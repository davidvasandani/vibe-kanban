# Prior knowledge — vk/bd71-require-all-poll

Searched Vibe Kanban `wiki/` and `docs/knowledge-base/` for poller and lifecycle knowledge.

- `vibe-kanban/wiki/vk-pollers.md`: pollers are BackgroundHelpers with optional PollerSpec JSON metadata, own process groups, raw logs, and restart re-adoption. Preserve the shared helper cap and migration-free legacy JSON reads.
- The same page documents nonzero tick continuation, EXIT-trap status capture, capped output, and API errors that must populate `message` for MCP clients.
- Poller sidebar details reuse streamed executions; do not add a polling request for stopping metadata.
- `vibe-kanban/wiki/agent-process-lifecycle.md` (linked from the poller page): turn-scoped children die at turn end; persistent helpers are independently tracked and stopped through process-group lifecycle controls.

Design consequence: embed stopping enforcement in the persistent execution itself, so server restart/re-adoption cannot reset a deadline. Extend existing metadata and UI projections rather than introducing a second scheduler.
