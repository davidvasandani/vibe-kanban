# Technical Spec: `list_all_messages` MCP tool

**Task:** `vk/29d8-vk-list-all-mess`

## Summary

Add a read-only `list_all_messages` tool to the Vibe Kanban MCP server alongside
`list_recent_messages`. The new tool returns the complete normalized message
history for either a session's latest coding-agent execution or a specific
execution, while preserving workspace scoping, role filtering, message order,
normalization, and per-message truncation.

## Motivation

`list_recent_messages` is intentionally a bounded tail reader: its default is
20 messages and the HTTP API clamps requests to 100. Orchestrators sometimes
need the complete conversation to recover earlier decisions and context. Asking
for a larger recent-message window cannot satisfy that requirement once a turn
contains more than 100 normalized messages.

## Functional requirements

1. Expose `list_all_messages` in both global and scoped/orchestrator MCP modes.
2. Accept exactly the same target choices as `list_recent_messages`:
   `session_id` (latest non-deleted coding-agent execution) or `execution_id`.
   At least one target is required; when both are supplied, preserve the
   existing execution-first behavior for compatibility.
3. Accept the same optional comma-separated `roles` filter.
4. Return the same response shape and message shape as
   `list_recent_messages`, ordered oldest to newest, but without a caller limit
   or the server's 100-message cap. `has_more` must be false for a successful
   all-messages response.
5. Reuse the existing normalized-log projection used by the UI and recent
   messages. Do not read raw logs, open a websocket, or introduce another data
   store.
6. Enforce the owning session's workspace scope before reading messages.
7. Preserve the existing per-message truncation and `final_message` behavior so
   an unbounded message count does not also make individual entries unbounded.
8. Leave `list_recent_messages` behavior and API compatibility unchanged.
9. Document when callers should choose recent versus all messages.

## Technical design

- Extend the messages HTTP query with an optional `all` boolean. The existing
  endpoints continue to clamp `limit` when `all` is absent or false. When
  `all=true`, the shared response builder retains every filtered normalized
  message and reports `has_more: false`.
- Generalize the MCP HTTP helper so it can request either a bounded `limit` or
  `all=true` while continuing to pass the optional role filter.
- Factor the shared target resolution and workspace authorization used by the
  two MCP tools where practical, avoiding behavior drift.
- Register the new tool through the sessions tool router and update the
  orchestrator allow-list test.

## Tests and acceptance criteria

- Unit tests prove bounded responses still tail and set `has_more`, while the
  all-messages mode returns more than 100 filtered messages in chronological
  order with `has_more: false`.
- MCP router tests prove `list_all_messages` is exposed to orchestrators.
- Existing recent-message tests remain green.
- Rust formatting and focused crate checks/tests pass.
- An independent Codex diff review reports no significant findings.

## Out of scope

- Changes to services other than Vibe Kanban.
- Combining messages across multiple executions in a session; session targeting
  continues to mean the latest coding-agent execution.
- Removing truncation, changing normalized-log semantics, or changing the
  existing `list_recent_messages` limit.
- Frontend UI changes or homelab deployment changes.
