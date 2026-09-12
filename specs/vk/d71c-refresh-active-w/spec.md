# Feature Specification: Refresh Active Workspace MCP Inventories

**Feature dir**: `specs/vk/d71c-refresh-active-w/`
**Task id**: `vk/d71c-refresh-active-w`
**Status**: Clarified

> The checked-in `/speckit.specify` prompt names
> `specs/vk/c89d-address-fable-fo/spec.md`, which is already owned by and
> populated for another completed task. In accordance with the repository's
> task-ownership contract, this run refused to overwrite that path and wrote to
> the current task-owned directory.

## Summary

Vibe Kanban must let an active workspace session adopt the current MCP tool
inventory after a connector is enabled or its advertised capabilities change.
The supported operation must refresh the agent-owned callable registry—not only
the connector UI's probe metadata—so added tools become callable and removed or
schema-changed tools are accurately represented on the next turn. When an
executor cannot safely reload in place, the product must offer a clear
same-workspace agent restart instead of requiring workspace recreation or
claiming a live refresh occurred.

## User Stories

- As a workspace user, I want a newly enabled stdio MCP tool to become callable
  in my existing task so I do not have to recreate the workspace.
- As a connector maintainer, I want tool additions, removals, and schema changes
  to replace the active registry atomically so the agent never uses a mixed or
  stale contract.
- As a user of an executor without proven live reload, I want a one-click,
  clearly labeled agent restart that preserves my workspace and conversation.
- As an operator, I want connector status and the agent's effective inventory to
  agree after refresh, with actionable and secret-safe failures when they do
  not.

## Functional Requirements

- **FR-1**: A refresh request for a workspace session must resolve the latest
  effective MCP configuration assigned to that session's executor profile.
- **FR-2**: For a remotely executed session, the refresh request must reach the
  worker and execution that own the active agent runtime.
- **FR-3**: A supported refresh must cause current MCP capabilities to be
  discovered again and must make the resulting callable definitions effective
  for the next model turn.
- **FR-4**: For a stdio MCP, capability rediscovery must observe the current
  process/configuration rather than reusing an inventory captured when the
  workspace session first started.
- **FR-5**: Tool additions, removals, and input-schema changes must replace the
  prior callable registry as one complete generation.
- **FR-6**: An acknowledged reload or successful connector probe must not be
  reported as adopted unless the process owning the coding-agent session
  provides the strongest confirmation its protocol supports.
- **FR-7**: Refresh must coordinate with active turns, tool calls, duplicate
  requests, and process start/exit handoffs so readers observe a deterministic
  pending, busy, successful, partially successful, unsupported, or failed state.
- **FR-8**: A failed or partial refresh must retain the last known usable
  callable inventory where the executor protocol supports retention; otherwise
  it must clearly require an agent restart rather than silently claiming
  success.
- **FR-9**: Executors without a proven safe live-reload contract must expose an
  explicit same-workspace agent restart/refresh action with clear messaging.
- **FR-10**: Restarting for MCP changes must preserve the logical workspace,
  conversation, queued user prompt, and normal continuation semantics, and must
  not be labeled an in-process reload.
- **FR-11**: The connector management UI must distinguish catalog suggestion,
  assigned native configuration, enabled state, connectivity test inventory,
  and active-agent adoption; it must not display contradictory installed status
  for one effective connector.
- **FR-12**: Connector inventory, refresh status, and agent-visible registry must
  use the same stable server identifiers and current effective assignment.
- **FR-13**: Public status and errors must remain secret-safe and must include a
  corrective next action.
- **FR-14**: Existing HTTP/SSE MCP behavior must remain unchanged unless direct
  evidence shows it shares the stale-lifecycle defect.

## Out of Scope

- Updating any service other than Vibe Kanban.
- Redesigning the plugin marketplace or treating catalog presence as native
  installation.
- Changing an in-flight model request's tool schema.
- Claiming protocol generation or per-server restart facts that the owning
  executor does not expose.

## Acceptance Criteria

- [ ] Starting one workspace session, changing a stdio MCP from inventory A to
  A+B, refreshing, and starting the next turn makes B callable without
  recreating the workspace.
- [ ] Changing A+B to B removes A from the next turn's callable registry.
- [ ] Changing B's input schema replaces the next turn's schema completely.
- [ ] Tests assert next-turn agent-visible definitions, not only connector probe
  results, tool counts, or reload acknowledgements.
- [ ] A failed refresh never yields a falsely successful or partially mixed
  registry and gives a secret-safe remediation.
- [ ] Unsupported live reload offers a one-click same-workspace agent restart
  with accurate confirmation and messaging.
- [ ] Connector assignment/status and the post-refresh active inventory agree
  by stable server identifier.
- [ ] At least one HTTP or SSE regression case passes unchanged.
- [ ] Focused backend/frontend tests, required generated-type checks, formatting,
  and repository verification pass.

## Open Questions

None. See `./clarifications.md` for the resolved boundaries.
