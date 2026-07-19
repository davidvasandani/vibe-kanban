# Shared MCP configuration

Contributing tasks: `a898-allow-mcp-server`, `4ae2-add-a-shared-mcp`,
`c3fb-add-slack-mcp-se`

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
checked before writing. Codex accepts stdio and streamable HTTP; legacy SSE
remains agent-native because Codex cannot consume that transport.

## Preconfigured server catalog

`crates/executors/default_mcp.json` is the canonical catalog for suggested MCP
servers. Keep entries transport-neutral in that file (`command`, `args`, and
`env` for stdio), use credential placeholders rather than secrets, and let
`mcp_config.rs` adapt the entry to each executor's native schema. In particular,
Opencode calls the stdio environment field `environment`; dropping or leaving it
as `env` makes credential-dependent catalog entries unusable after adaptation.

The backend exposes this catalog through `/api/mcp-config/default`, but the
current shared MCP settings UI does not render catalog suggestions. Treat catalog
availability and UI discoverability as separate capabilities when scoping work.

## Shared gateway authentication

OAuth-capable streamable HTTP assignments can use the local Vibe MCP gateway.
OAuth is completed once per user, host, server name, and canonical upstream URL;
all assigned agents receive the same loopback `/mcp-gateway/{connection_id}` URL
and an unguessable local bearer capability. Upstream access and refresh tokens
are encrypted in SQLite with a host-local AES-GCM key and never enter agent
configuration files, API responses, or logs.

The gateway validates the real socket peer through Axum `ConnectInfo` (not the
spoofable `Host` header), compares capability hashes in constant time, and binds
on the existing local server address. It forwards only MCP-relevant headers,
replaces Authorization with the upstream token, streams responses, refreshes
tokens centrally, and rejects unsafe upstream destinations and redirects.

Read models redact the local capability as `Bearer [REDACTED]`. Saves hydrate
that placeholder from native snapshots only while the gateway URL still
matches; changing to a direct URL with the placeholder fails closed. Reconnects
reuse the capability and deterministic connection ID, compare canonical URLs,
and match the gateway path across changing local ports. Removing one assignment
only edits that agent's native config. Disconnecting revokes refresh and access
tokens where supported and disables every remaining assignment.

Cloudflare Access service-token headers are encrypted with the OAuth token set
and sent only to the configured MCP origin, never to a different authorization
server origin. Interactive discovery redirects produce actionable guidance; the
UI retains that error so the next Connect attempt can request the service token.
