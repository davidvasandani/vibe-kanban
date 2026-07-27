# Technical Plan: Active Workspace MCP Refresh

## Constitution check

- Principle II: route, coordinator, protocol, UI, concurrency, and redaction
  behavior receive contract tests.
- Principle III/VI: the first release is Codex-only and reuses the pinned
  app-server refresh/status protocol rather than building an MCP client manager.
- Principle IX: live behavior stays behind a shared executor-control abstraction
  with a capability-gated vendor adapter.
- Principle XII: the per-session coordinator owns the queued-to-confirmed
  handoff; no coordination lock is held across JSON-RPC requests.
- Principle XVII: success requires live Codex confirmation and inventory state is
  published as a complete generation.

No constitution deviation or new top-level dependency is planned.

## Architecture

### Executor live-control handoff

Extend `SpawnedChild` with an optional receiver that yields a type-erased
`McpRefreshControl` after the executor has initialized its control protocol.
Codex supplies an implementation backed by `Arc<AppServerClient>`. The container
registers it by session/execution while the child is alive and removes it during
finalization.

The control trait supports:

- capability identification;
- queueing an MCP config reload;
- fetching a complete paginated MCP status inventory.

Other executors return no control and are unsupported.

### Session coordinator

Add `McpRefreshCoordinator` to `crates/services` (domain types, state machine,
redaction) and store its per-session state/control registrations in
`LocalContainerService`.

The refresh route asks `ContainerService::refresh_mcp_tools`. The local
implementation:

1. validates the latest executor profile is Codex-capable;
2. atomically claims the session refresh state;
3. sends `config/mcpServer/reload` through a live control if present, otherwise
   records a pending generation to be confirmed on the next Codex execution;
4. never interrupts the running turn;
5. rejects a second claim as retryable busy.

When a Codex control registers for the next execution, the container confirms
the queued generation after thread/turn startup by fetching all status pages.
It transforms only safe fields, merges failed servers with last known-good
snapshots, then swaps the complete map under one write lock.

### API and MCP

Add GET/POST session refresh routes under the workspace router. Add the
`refresh_mcp_tools` VK MCP tool that forwards to the route/service using existing
orchestrator scoping.

### UI

Add API methods and a compact refresh-status control to the active session
header/control area in `packages/web-core`. The button request begins polling
while pending and stops at a terminal generation. The detail popover/dialog
shows confirmed time and server rows. Unknown restart/count values render as
unknown, never false/zero.

## Security

The public model is allow-listed from Codex typed status fields. Raw JSON-RPC
errors are mapped by category and never serialized/logged verbatim. Server IDs
are bounded/sanitized for display. No config definition or status auth payload
crosses the boundary.

## Testing

- Unit-test coordinator claim/confirm, concurrent refresh, whole and partial
  failure, last-known-good retention, disable/removal, and atomic reads.
- Fixture-test Codex client request serialization and paginated status mapping.
- Container tests cover control registration/removal and pending-next-turn
  confirmation.
- Route tests cover scope and domain statuses.
- MCP router/tool tests cover global/orchestrator scoping.
- UI rendered-DOM tests cover pending, success, partial, busy, unsupported, and
  unknown fields.
- Deterministic stdio and streamable-HTTP fixtures exercise additions/removals,
  malformed tools, timeout, and in-flight calls through the pinned Codex layer
  where feasible.
- An ignored live Slack test verifies `attachment_get_data` against
  `v1.3.0-vk.2` with isolated caches.

## Implementation order

1. Domain types/coordinator/redaction.
2. Executor control trait and Codex protocol methods.
3. Container registration, request, next-turn confirmation, and teardown.
4. REST routes and generated types.
5. VK MCP tool.
6. Web API/UI.
7. Integration fixtures, Slack regression, docs, and full verification.
