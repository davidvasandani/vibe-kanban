# Feature Specification: Recover Wedged MCP Sessions

**Feature dir**: `specs/vk/38a6-expose-session-w/`
**Status**: Clarified

## Summary

Vibe Kanban will let users and agents restart an active coding-agent session or
its workspace runtime when one or more MCP servers are wedged. Restarts preserve
conversation and repository state, rediscover MCP capabilities from a fresh
process, and produce bounded, truthful per-server outcomes. The active-session
MCP view will distinguish transport connectivity from registered tools and offer
an explicit, prefilled Vibe Kanban issue action when recovery fails.

## User Stories

- As an agent in a session with a wedged MCP server, I want to restart my own
  session so I can regain the missing capability without losing context.
- As a user, I want a heavier workspace restart so I can recover all processes
  owned by a wedged workspace without losing files or Git state.
- As a user, I want every MCP server to become usable or fail terminally within
  a bounded time so I am not told it is “still connecting” forever.
- As a user, I want the MCP panel to show what the active agent actually
  registered so a green transport check cannot hide a missing tool inventory.
- As a user, I want a one-click, prefilled issue draft when MCP recovery fails so
  maintainers receive the evidence needed to diagnose it.

## Functional Requirements

- FR-1: The Vibe Kanban MCP surface MUST expose `restart_session` with an
  optional session identifier.
- FR-2: In scoped context, `restart_session` MUST default to the current session;
  outside scoped context it MUST require an explicit session identifier.
- FR-3: A session restart MUST preserve its conversation and start a fresh agent
  process that performs MCP discovery from the beginning.
- FR-4: A session MUST be able to request its own restart and receive an accepted
  result before the current process ends; Vibe Kanban MUST resume the session
  afterward.
- FR-5: The Vibe Kanban MCP surface MUST expose `restart_workspace` with an
  optional workspace identifier.
- FR-6: In scoped context, `restart_workspace` MUST default to the current
  workspace; outside scoped context it MUST require an explicit workspace
  identifier.
- FR-7: A workspace restart MUST restart the processes owned by that workspace
  while preserving its worktree, uncommitted changes, Git metadata, sessions,
  and conversations.
- FR-8: A workspace restart requested from inside the workspace MUST be accepted
  before the caller is stopped and MUST resume the requesting session afterward.
- FR-9: Duplicate restart requests for the same target MUST NOT create duplicate
  continuation processes.
- FR-10: Restart status MUST include the target, restart disposition, timestamps,
  and a per-server MCP outcome.
- FR-11: Each configured MCP server MUST reach either a usable state or a named
  terminal failure within a bounded interval. Repeated transient errors MUST NOT
  extend that interval indefinitely.
- FR-12: Each per-server outcome MUST identify the server, lifecycle state,
  registered tool count including zero, safe terminal error where applicable,
  observed error codes with timestamps, and discovery-attempt count.
- FR-13: Per-server errors and diagnostics MUST exclude credentials,
  authorization material, secret environment values, and authenticated URLs.
- FR-14: `refresh_mcp_tools` MUST explicitly report whether the selected executor
  supports live refresh and whether a refresh was queued or applied.
- FR-15: The product MUST explicitly document and test the live-refresh behavior
  for `CLAUDE_CODE`; an unsupported executor MUST return a terminal unsupported
  result rather than apparent success.
- FR-16: The MCP panel MUST expose active-session registry state separately from
  saved-configuration connectivity tests.
- FR-17: The MCP panel MUST NOT call a server usable unless the active executor
  completed discovery. It MUST explicitly show a connected server with zero
  registered tools.
- FR-18: If a server is terminally unusable, exceeds its discovery deadline, or
  has transport/registry disagreement, the UI MUST offer to open a prefilled
  Vibe Kanban issue.
- FR-19: The issue draft MUST include server names; safe error codes and
  timestamps; executor, workspace, and session identifiers; servers that did
  register with tool counts; and discovery-attempt counts.
- FR-20: Detection alone MUST NOT create an issue. Creation requires an explicit
  user action and MUST prevent duplicate submissions.
- FR-21: Restart and status operations MUST enforce the same workspace/session
  scoping rules as existing scoped MCP tools.
- FR-22: Local and clustered workspaces MUST report the same public restart and
  discovery contract, routed through the process owner responsible for the
  target workspace.

## Out of Scope

- Repairing credentials, network access, or implementation defects in any
  third-party MCP server.
- Restarting a Vibe Kanban deployment, cluster coordinator, or worker service.
- Restarting unrelated homelab services.
- Automatically submitting an issue without user confirmation.
- Guaranteeing that restart makes an unhealthy third-party server healthy.

## Acceptance Criteria

- [ ] The MCP tool inventory includes documented `restart_session` and
  `restart_workspace` tools with current-context defaults.
- [ ] Calling `restart_session` inside its target session preserves prior
  conversation and ends with every configured server usable or terminally
  failed.
- [ ] Calling `restart_workspace` preserves files, uncommitted changes, Git
  state, and conversation while replacing workspace-owned processes.
- [ ] Repeating either restart during an active restart does not start a second
  continuation.
- [ ] `refresh_mcp_tools` produces an explicit unsupported result for every
  executor without verified live refresh, with `CLAUDE_CODE` covered by a test.
- [ ] A server emitting repeated `CONNECT_TIMEOUT` and `CONNECTION_CLOSED`
  reaches a named terminal failure within the configured bound.
- [ ] The MCP panel cannot show a server as usable when the active registry has
  no registered inventory; zero tools is displayed explicitly.
- [ ] An unusable server exposes a prefilled issue action whose draft contains
  the required safe diagnostic bundle and whose creation requires a click.
- [ ] Automated tests cover scoped defaults, authorization, lifecycle handoff,
  deadline behavior, diagnostic redaction, and UI state reconciliation.

## Clarified Decisions

- **Running-turn behavior:** A self-restart is accepted and queued, the calling
  turn is allowed to return its tool response and exit normally, and the fresh
  continuation starts immediately afterward. It does not asynchronously kill
  the process before acknowledgement. An external restart request follows the
  same safe handoff unless a separate, explicitly destructive stop action is
  used.
- **Workspace process boundary:** Restart every active process whose lifecycle is
  owned by the workspace container (coding-agent processes, warm executor
  servers, dev servers, and background helpers/pollers). Recreate processes that
  already have durable launch definitions; resume the requesting coding-agent
  session through a fresh continuation. If an active process cannot be safely
  recreated from durable state, report it as a named partial failure rather than
  guessing a command or claiming full success.
- **Zero-tool servers:** Successful discovery with zero tools is a terminal,
  non-error `connected_no_tools` state. It may be valid for a resource-only or
  prompt-only server, but it is never displayed as evidence that tools are
  available. Resource and prompt counts remain visible when the executor
  protocol supplies them.
