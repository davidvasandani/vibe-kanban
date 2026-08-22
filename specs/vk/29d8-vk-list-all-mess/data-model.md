# Data Model: `list_all_messages`

No persistent data model changes.

## Existing projection

- `RecentMessagesPayload` / `ListRecentMessagesResponse`
  - session and execution identifiers
  - execution status and optional exit code
  - optional final assistant message
  - ordered `RecentMessage` list
  - `has_more`
- `RecentMessage`
  - stable execution-scoped id
  - normalized role and truncated text
  - optional timestamp
  - execution identifier

## New transient selector

- `MessagesSelection::Recent(limit)` selects the newest bounded entries and
  reports whether earlier filtered entries exist.
- `MessagesSelection::All` retains all filtered entries and reports
  `has_more = false`.

The MCP all-message response deliberately reuses the existing response DTO.
