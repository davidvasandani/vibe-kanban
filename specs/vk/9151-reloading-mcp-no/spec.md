# Feature Specification: Reliable MCP Reload

**Feature dir**: `specs/vk/9151-reloading-mcp-no/`
**Status**: Draft

## Summary

Make the existing workspace-chat MCP reload control reliably apply newly saved
MCP configuration to the next suitable turn in the same conversation. Users
need an honest queued/applied/failure result and must not have to replace the
conversation to gain newly configured tools.

## User Stories

- As a user who changed MCP settings, I want to reload the active session so my
  next turn can use the new tools without losing conversation context.
- As a user waiting for a reload, I want to know whether it is queued, applied,
  unsupported, busy, partially applied, or failed so I know what to do next.
- As a user moving between sessions, I want reload feedback to remain attached
  to the session where I requested it.

## Functional Requirements

- FR-1: A reload request for a supported session must create one session-scoped
  refresh generation.
- FR-2: The browser must recover and display an existing pending generation
  whenever its session is selected or remounted.
- FR-3: A duplicate request that receives a transient busy response must
  reconcile the canonical generation and continue tracking it to completion.
- FR-4: The system must report success only after the capability-owning process
  confirms its post-request MCP server inventory.
- FR-5: The browser must continue polling a canonical pending generation until
  the backend reports a terminal result or the user selects another session.
- FR-6: Concurrent requests for one pending generation must not create
  overlapping reloads or advance the generation more than once.
- FR-7: Unsupported, partial, busy, and failed backend outcomes must retain
  their accurate existing meaning in the browser rather than being converted
  to false success or indefinite local pending state.
- FR-8: Browser-visible failure details must be sanitized and must not expose
  credentials, environment values, authenticated URLs, command arguments, or
  raw subprocess output.
- FR-9: Reloading must preserve the conversation and session identity.
- FR-10: Reload status and polling must remain isolated to the active session;
  a late response from another session must not overwrite it.
- FR-11: Terminal or retryable outcomes must leave the control in an appropriate
  state for a later retry, while a pending generation prevents duplicate input.

## Out of Scope

- Adding live reload support to executors without a proven refresh contract.
- Restarting or replacing the user-visible conversation.
- Editing MCP definitions, credentials, or assignments as part of reload.
- Modifying services other than Vibe Kanban or deployment files other than its
  governing `modules/vibe-kanban-rebuild.nix` module.

## Acceptance Criteria

- [ ] A regression test demonstrates that selecting/remounting a session with a
      pending reload hydrates that state and tracks it to a terminal result.
- [ ] A duplicate click that receives `busy` immediately reconciles the stored
      pending generation and tracks it to a terminal result.
- [ ] Repeated requests while pending return a busy/idempotent result without
      creating a second generation.
- [ ] Unsupported and failed boundaries expose safe actionable outcomes and do
      not remain pending.
- [ ] Session changes cannot display a late reload result from the previous
      session.
- [ ] The existing conversation ID remains unchanged through request, next
      turn, and confirmation.
- [ ] Targeted backend and frontend verification plus repository formatting
      pass.

## Open Questions

None.
