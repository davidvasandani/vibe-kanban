# Technical plan

Applied `/speckit.plan`. Rust/SQLite own runtime archive admission; PostgreSQL owns remote completion intent. Reuse the delivery ledger and ContainerService cleanup.

1. Add a local archive-state table for durable trigger identity, rule, occurrence time, and explicit-unarchive acknowledgement. Use a stable remote archive event identity, not ordinary workspace updated_at, which changes on metadata edits. Keep provenance independent of generated Workspace projections.
2. Extend automatic admission to persist intent and protect acknowledged triggers. Route PR monitor and both merge handlers through it. Retry deferred intents from the existing monitor loop.
3. Extend wake-up insertion's immediate transaction to verify workspace/session and reactivate only after limits pass; preserve pending intent. Improve errors at the API boundary.
4. Inject VK_SESSION_ID beside VK_WORKSPACE_ID on local and worker launch. Resolve omitted MCP targets from the paired environment identity, with backend session ownership validation. Retain explicit external targeting.
5. Return provenance through a dedicated workspace archive-state API and include it in MCP listing and an existing workspace UI surface.
6. Add database, API and MCP regression coverage; regenerate types if needed, format, run targeted checks and independent review.

Constitution check: reuse existing state machines (III, VI), transactional remote intent (V), attributable diagnostics (XI), atomic local decisions (XII). No exception required.
