# Shared MCP configuration

Contributing tasks: `a898-allow-mcp-server`

Vibe Kanban derives shared MCP settings from each base executor's native config
file. There is no separate registry: the native files remain the source consumed
by Claude Code, Codex, Gemini, Opencode, Cursor, Copilot, and other agents.

## Backend shape

- Shared domain logic lives in `crates/executors/src/shared_mcp_config.rs`.
- `GET /api/mcp-config/shared` reads every MCP-capable base executor, normalises
  native entries, merges same-name equivalent definitions, and reports conflicts
  when same-name entries differ.
- `POST /api/mcp-config/shared` accepts the complete logical server list and
  materialises each assignment back into that executor's native shape.
- `POST /api/mcp-config/shared/test` reads saved native entries from disk and
  returns results keyed by both logical server name and executor.

## Write semantics

Each native profile is an independent write target. A multi-profile save reports
per-profile outcomes and can partially succeed; successful native writes remain
committed if a later profile fails. Individual file writes are staged and then
renamed into place, and the previous version is retained beside the config file
with a `.bak` suffix for independent recovery.

## Compatibility

Assignments target base executor types only. Named variants and per-task
executor overrides are not separate MCP assignment targets. Compatibility is
checked before writing; Codex is stdio-only, so URL-based definitions are blocked
for Codex instead of being dropped by its native adapter.

## OAuth

OAuth remains assignment-scoped. The existing `/api/mcp-auth/*` routes receive a
specific `executor` and `server_name`; completion writes credentials only to that
native entry. The frontend reloads the shared model after OAuth completion so a
later ordinary save does not overwrite the freshly written credential.
