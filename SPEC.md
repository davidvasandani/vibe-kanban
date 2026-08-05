# Technical Specification: Reliable MCP Reload for Existing Sessions

## Problem

The workspace chat toolbar exposes a control that promises to reload MCP
configuration without replacing the current conversation. In practice, using
the control does not reliably make newly configured MCP servers and tools
available to the next Codex turn. The UI may remain queued while the session
continues with the old MCP inventory, leaving the operator without a dependable
reload or a useful failure result.

## Scope

This change is limited to the Vibe Kanban service repository and, only if the
runtime packaging proves relevant, its deployment definition in
`homelab/modules/vibe-kanban-rebuild.nix`.

The investigation and fix cover:

- the chat toolbar reload request and status feedback;
- server-side refresh coordination across the boundary between executions in
  one session;
- Codex app-server MCP configuration reload and inventory confirmation;
- regression tests for the failing lifecycle.

Changes to other services are explicitly out of scope.

## Required Behavior

1. Clicking the reload control for a Codex-backed session records exactly one
   refresh generation and gives immediate, accurate feedback.
2. If a live Codex app-server control can accept the reload, Vibe Kanban queues
   it safely; otherwise the request remains eligible for the next execution.
3. The next agent turn in the same conversation must start from or adopt the
   refreshed MCP configuration before Vibe Kanban reports success.
4. Success must be confirmed from the MCP server inventory associated with the
   post-request execution boundary, never from a stale pre-request inventory.
5. A reload requested while no execution is live must not remain permanently
   pending once the next turn starts.
6. Unsupported executors and actual initialization, authentication, capability,
   or process failures must produce bounded, actionable status instead of a
   false success or an indefinite pending state.
7. Repeated clicks while a generation is pending must remain idempotent and must
   not create overlapping refreshes.
8. Navigating between sessions must not display or apply another session's
   refresh result.

## Acceptance Criteria

- An automated regression test reproduces the lifecycle responsible for the
  reported failure and fails without the fix.
- A reload requested between turns is confirmed or fails after the following
  Codex execution initializes; it does not stay `pending_next_turn`.
- A reload requested during the relevant startup race is applied at the first
  safe boundary and confirmed no earlier than that boundary.
- The frontend presents queued, successful, partial, unsupported, busy, and
  failed outcomes consistently and re-enables retry when appropriate.
- Existing conversation/session identity is preserved throughout.
- Targeted Rust and frontend checks pass, generated types remain synchronized,
  and formatting/lint relevant to changed files pass.

## Non-Goals

- Adding live MCP refresh support to non-Codex executors.
- Replacing or restarting the user-visible conversation.
- Changing MCP server definitions or credentials.
- Modifying unrelated homelab services.

## Risks and Constraints

- Codex app-server controls are execution-scoped today, while the operator's
  intent is session-scoped. Ordering must therefore be based on an explicit
  execution boundary and not merely on whether a control happens to exist.
- Refresh status is process-local coordinator state; cleanup and startup races
  can otherwise strand or incorrectly confirm a generation.
- Errors exposed to the browser must remain sanitized and must not leak MCP
  credentials or command details.
