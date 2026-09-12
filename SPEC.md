# Technical Specification: Recover Wedged MCP Sessions

## Objective

Give an active Vibe Kanban coding-agent session a bounded, observable recovery
path when one or more configured MCP servers never become usable. Recovery must
preserve the conversation, worktree, and Git state, and it must be callable from
the session that needs recovery.

## Scope

This feature changes only the Vibe Kanban repository. Deployment changes, if
required by implementation evidence, are limited to the service's governing
`homelab/modules/vibe-kanban-rebuild.nix` module.

The feature includes:

1. MCP tools `restart_session` and `restart_workspace`.
2. Backend restart operations that preserve durable session/workspace state.
3. A bounded MCP discovery lifecycle with explicit per-server terminal status.
4. Accurate reporting of active-session registry state in the MCP settings UI.
5. An action that offers to create a prefilled Vibe Kanban issue containing MCP
   diagnostics when a configured server is unusable.
6. Explicit unsupported-executor results from `refresh_mcp_tools`, including a
   confirmed behavior contract for `CLAUDE_CODE`.

## Functional Requirements

### Restart session

- Expose `restart_session(session_id?)` from the Vibe Kanban MCP server.
- In orchestrator/scoped mode, omitted `session_id` resolves to the current
  session. In global mode, the identifier is required.
- Restart the session's coding-agent executor process and start a continuation
  turn from the existing durable conversation rather than creating a new
  conversation or deleting prior execution records.
- A call originating inside the target session must acknowledge and schedule
  the restart through backend-owned lifecycle control so the tool response is
  not lost when the caller's process exits.
- Re-run MCP discovery from a clean executor process.
- Return the restart disposition and per-server discovery outcomes, including
  server name, lifecycle state, registered tool count, terminal error code and
  message where applicable, and timestamps.

### Restart workspace

- Expose `restart_workspace(workspace_id?)` from the Vibe Kanban MCP server.
- In orchestrator/scoped mode, omitted `workspace_id` resolves to the current
  workspace. In global mode, the identifier is required.
- Stop and recreate the workspace-owned process group without deleting or
  recreating its worktree, repository checkout, conversation, or Git state.
- Resume the current session after the backend has safely transferred restart
  ownership away from the calling executor.
- Return the same per-server MCP outcome shape as `restart_session`.

### MCP discovery state

- Model configured servers with explicit states at least equivalent to
  `connecting`, `usable`, and `failed`.
- A server is usable only when the active executor's tool registry has completed
  discovery; status must include the number of registered tools, including zero.
- Connection attempts must reach `usable` or a named terminal failure within a
  bounded, configured deadline. Repeated `CONNECT_TIMEOUT` or
  `CONNECTION_CLOSED` events cannot reset that deadline indefinitely.
- Preserve an event summary containing observed error codes and timestamps and
  the count of tool-discovery attempts.
- The `/mcp` UI must display the active session registry state. A transport-level
  connection cannot be presented as equivalent to tools being registered.

### Refresh behavior

- `refresh_mcp_tools` must report whether the selected executor supports active
  refresh and whether the refresh was actually queued/applied.
- Unsupported executors return an explicit terminal unsupported result. The
  implementation and documentation must state whether `CLAUDE_CODE` supports
  refresh, backed by an executor-level test.

### Issue offer

- When MCP discovery reaches a terminal unusable state, or when transport state
  and registry state disagree, the UI surfaces a non-blocking offer to open a
  Vibe Kanban issue.
- The issue draft includes server names, error codes and timestamps, executor,
  workspace and session IDs, successfully registered servers with tool counts,
  and discovery-attempt counts.
- Opening the draft requires a user action; detection alone must not create an
  external issue.

## Safety and State Guarantees

- Restart operations are idempotent while a restart is already pending/running.
- Durable transcript, execution history, worktree files, and Git metadata remain
  intact across either restart.
- Backend-owned orchestration must prevent the calling process from killing the
  recovery operation as a side effect of its own termination.
- Authorization and orchestrator workspace scoping match existing session and
  workspace tools; callers cannot restart resources outside their allowed scope.
- Responses and UI diagnostics must not include MCP credentials, headers, token
  values, or raw environment variables.

## API and Contract Direction

- Add backend routes for session and workspace restart plus a read model for MCP
  discovery status. Exact paths and payload names will be finalized by SpecKit.
- Add shared Rust/TypeScript response types generated through the existing type
  generation path; generated files are not edited manually.
- MCP tool results use structured JSON payloads rather than success-only text so
  agents can distinguish recovery, terminal failure, and unsupported behavior.

## Verification

- Unit tests cover default-context resolution, scope rejection, idempotency,
  unsupported refresh behavior, deadline transition, status aggregation, and
  secret redaction.
- Integration tests simulate a healthy server, clean authentication failure,
  repeated timeout/connection-close failure, and a connected server that
  registers zero tools.
- A self-restart test invokes `restart_session` from the target session and
  verifies that a continuation retains prior conversation while discovery ends
  in either usable or named terminal state.
- Workspace restart tests verify unchanged worktree and Git state.
- UI tests verify registry-accurate labels and the prefilled issue action.

## Out of Scope

- Repairing Slack, Brink, Cloudflare, or Atlassian credentials or server
  implementations.
- Restarting unrelated homelab services or hosts.
- Automatically filing an issue without user confirmation.
- Guaranteeing a third-party MCP server becomes healthy; the guarantee is a
  bounded, truthful terminal outcome.

## Acceptance Criteria

The implementation is complete when all acceptance criteria in the task are
covered by automated tests or an explicit, reproducible verification, the
project documentation describes both restart tools and executor refresh support,
an independent Codex review reports no significant findings, reusable knowledge
is recorded, and the resulting pull request is merged into the base branch.
