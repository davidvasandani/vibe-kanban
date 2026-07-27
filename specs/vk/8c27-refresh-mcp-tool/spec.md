# Feature Specification: Refresh MCP Tools in Active Workspace Sessions

**Feature dir**: `specs/vk/8c27-refresh-mcp-tool/`  
**Status**: Draft

## Summary

Users and agents need an explicit way to refresh the MCP capabilities available
inside an active Vibe Kanban workspace session after MCP servers or credentials
change. Refresh must preserve the workspace and conversation, make the confirmed
new inventory available to the next turn, isolate individual server failures,
and never expose secret configuration.

## User Stories

- As a workspace user, I want to refresh MCP tools from the active session so
  that newly installed or updated tools are usable without losing conversation
  history.
- As a workspace agent, I want to request the same refresh programmatically so
  that I can recover after a connector update or credential renewal.
- As a user managing several MCP servers, I want healthy unchanged servers to
  stay connected when one server changes or fails.
- As a user diagnosing refresh failure, I want a classified per-server result
  and safe remediation without credentials appearing in the UI or logs.
- As a user invoking tools concurrently, I want refresh to avoid interrupting a
  call that is already running.

## Functional Requirements

- **FR-1:** An active workspace session MUST offer a user-visible **Refresh MCP
  tools** action.
- **FR-2:** The same session-scoped operation MUST be callable through the Vibe
  Kanban API and through the Vibe Kanban MCP surface where session scope is
  available.
- **FR-3:** Refresh MUST preserve the workspace, session, conversation history,
  repository state, and user-visible transcript.
- **FR-4:** Refresh MUST re-evaluate the MCP configuration applicable to the live
  session, including added, removed, enabled, disabled, changed, and
  credential-renewed servers.
- **FR-5:** Refresh MUST distinguish unchanged healthy servers that can be reused
  from servers that require restart, reconnect, or reinitialization.
- **FR-6:** Affected servers MUST complete initialization and capability
  enumeration before their refreshed capabilities are reported as active.
- **FR-7:** The next agent turn MUST receive the confirmed refreshed inventory.
- **FR-8:** An agent MUST observe either the complete previous inventory or the
  complete refreshed inventory, never an intermediate or partially constructed
  inventory.
- **FR-9:** Refresh MUST NOT interrupt an in-flight tool call. It MUST either
  serialize safely behind the call or return a clear retryable busy result.
- **FR-10:** Concurrent refresh requests MUST be serialized or rejected with a
  clear retryable busy result.
- **FR-11:** Failure of one server MUST NOT make healthy servers unusable.
- **FR-12:** When a changed server fails to refresh, its last known-good
  capabilities MUST remain available until a later successful refresh or the
  server is explicitly disabled.
- **FR-13:** A server that is explicitly removed or disabled MUST disappear from
  the confirmed inventory after a successful refresh.
- **FR-14:** A later call to a removed tool MUST return a clear unavailable-tool
  result.
- **FR-15:** Refresh results MUST identify each server by its configured
  identifier and report overall and per-server status.
- **FR-16:** Results MUST show the last successful refresh time, discovered tool
  count, and whether each server connection was reused, restarted, reconnected,
  added, removed, disabled, or retained after failure.
- **FR-17:** The product MUST NOT present refresh as successful until it has
  confirmation that the live session will serve the refreshed inventory.
- **FR-18:** Refresh failures MUST distinguish at least:
  executable/package unavailable, process launch failure, initialization or
  handshake failure, authentication failure, capability-list failure, invalid
  tool schema, timeout, refresh already in progress, active-call busy, and
  unsupported live refresh.
- **FR-19:** Failure results MUST include safe remediation appropriate to the
  failure category.
- **FR-20:** Errors, logs, API results, and UI content MUST NOT expose tokens,
  environment-variable values, OAuth material, authorization/cookie values,
  secret command arguments, or raw authenticated URLs.
- **FR-21:** If a live executor cannot confirm in-session reload, the operation
  MUST report unsupported and MUST NOT claim success based on an independent
  connectivity test or a replacement conversation.
- **FR-22:** Capability reporting SHOULD include resource and prompt counts when
  the live executor makes those capabilities observable; unavailable counts
  MUST remain unknown rather than be reported as zero.

## Out of Scope

- A new extension to the MCP protocol for third-party clients.
- Automatically changing MCP server configuration from the refresh action.
- Replacing an unsupported agent conversation and presenting that replacement
  as an in-session refresh.
- Displaying raw subprocess output or authenticated network responses.
- Guaranteeing resource or prompt counts for executors that do not expose them.

## Acceptance Criteria

- [ ] Start one workspace session with a server exposing tool set A, update the
  same configured server to expose A+B, refresh, and call B on the next turn
  without changing workspace/session identity or losing transcript history.
- [ ] Change A+B back to A, refresh, and verify B disappears and a later
  reference returns an unavailable-tool result.
- [ ] Add, remove, enable, and disable an MCP server and observe each change
  after refresh in the same session.
- [ ] Renew credentials for an authentication-failed server and recover it with
  refresh without restarting the workspace.
- [ ] Refresh with one failing server and verify healthy servers remain usable
  while the failed server retains its last known-good inventory.
- [ ] Explicitly disable the failed server and verify its retained inventory is
  removed.
- [ ] Trigger refresh during an in-flight call and verify the call completes
  without interruption while refresh waits or returns a retryable busy result.
- [ ] Trigger two refreshes concurrently and verify they serialize or one
  receives a retryable busy result.
- [ ] Verify readers racing refresh observe only a complete old or complete new
  inventory.
- [ ] Exercise stdio and streamable-HTTP servers for tool addition/removal,
  timeout, malformed `tools/list`, partial failure, and secret redaction.
- [ ] Verify the UI displays last successful time, per-server status, tool count,
  and restart/reuse state and does not show success for stale inventory.
- [ ] From an existing workspace session, switch/reload Slack to the pinned fork
  `v1.3.0-vk.2`, refresh, and confirm `attachment_get_data` is callable without
  creating a new workspace session.
- [ ] Verify every required error category is returned with the configured
  server identifier and safe remediation, with injected secrets absent from
  response bodies and captured logs.

## Clarified Decisions

- The first release supports Codex app-server sessions, whose pinned protocol
  exposes `config/mcpServer/reload` and MCP status enumeration. Other executors
  return an explicit capability-gated `unsupported` result until a comparable
  vendor contract is proven.
- Initial support need not cover every MCP-capable executor, but the UI and API
  must never hide or mislabel unsupported status.
- Refresh metadata is live-session state in the first release. It need not
  survive a VK backend restart because the associated live process/control
  channel also does not survive that restart.
- Refresh requested during an active call is queued for Codex's next active turn,
  matching the vendor contract and avoiding interruption. A concurrent refresh
  request returns a retryable busy response. Completion is reported only after
  the next-turn inventory is confirmed; the initial request may report
  `pending_next_turn`.
