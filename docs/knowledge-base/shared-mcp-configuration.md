# Shared MCP configuration

Contributing tasks: `a898-allow-mcp-server`, `4ae2-add-a-shared-mcp`,
`c3fb-add-slack-mcp-se`, `76d1-vk-mcp-ux`, `d893-fix-slack-mcp`,
`067cb434-mcp-tools`

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

Frontend assignment compatibility is not always identical to the native-entry
form codec's transport list. The Codex form codec only parses and serializes its
stdio editor shape, while shared materialization also adapts canonical HTTP
definitions to Codex's `url`/`http_headers` shape. Assignment pickers must follow
the shared materialization contract (including Codex HTTP and excluding Codex /
Grok legacy SSE), not assume that an editor codec's narrower surface is the full
backend capability set.

## Management UI state boundary

The MCP server inventory is a read-oriented management surface: cards show the
logical server, transport, assigned agents, connection/auth state, and explicit
Test/Edit/Delete actions. Agent assignment controls belong in the add/edit
dialog with the server definition fields.

The dialog owns provisional definition and assignment state. NiceModal reuses
mounted components, so every open must re-seed all editable fields and
assignments. Cancel/close resolves without a result and leaves the outer draft
untouched; submit returns the complete `{ name, entry, assignments }` result.
Transport changes remove assignments that are no longer compatible, and submit
requires at least one assignment. The settings-level Save/Discard boundary
continues to persist or roll back the confirmed outer draft.

## Preconfigured server catalog

`crates/executors/default_mcp.json` is the canonical catalog for suggested MCP
servers. Keep entries transport-neutral in that file (`command`, `args`, and
`env` for stdio), use credential placeholders rather than secrets, and let
`mcp_config.rs` adapt the entry to each executor's native schema. In particular,
Opencode calls the stdio environment field `environment`; dropping or leaving it
as `env` makes credential-dependent catalog entries unusable after adaptation.

A catalog entry's `meta.<server>.url` is a link shown in the UI; it has no
effect on what the `command`/`args` actually install. When an entry points at a
fork or any non-obvious build, keep the two in sync deliberately and assert it —
see [forked-mcp-server-packaging](forked-mcp-server-packaging.md).

The backend exposes this catalog through `/api/mcp-config/default`, but the
current shared MCP settings UI does not render catalog suggestions. Treat catalog
availability and UI discoverability as separate capabilities when scoping work.

Catalog presence also does not materialize an MCP server into an executor's
native configuration. Codex discovers servers from the `config.toml` belonging
to the exact identity and `CODEX_HOME` that launch `codex app-server`. For a
deployment-owned server, seed that native configuration under the service
identity before Vibe Kanban starts. `codex mcp add <name> -- <command>` is an
idempotent named-table update: it preserves unrelated Codex settings and is
safer than generating the whole TOML file. Use immutable deployment paths and
put environment-specific private endpoints in deployment configuration, not in
the global product catalog.

Configuration alone is not proof of availability. Diagnose the complete
boundary in order: configured server names for the launch identity, app-server
startup/handshake status, registered tool counts, and the worker that actually
owns the workspace process. Historical workspace rows may be insufficient once
their process or worker-affinity record has been removed, so retain or surface
startup failures and server-status snapshots while the session is live.

Catalog changes do not rewrite native executor files that were saved from an
older bundled template. If a later immutable pin makes those files appear to
conflict with a current profile, handle the transition at the shared read
boundary with a server-name-aware, exact historical-template migration. Match
the complete old command, ordered arguments, and environment-key shape; source
the replacement from the current catalog; and preserve the stored credential.
Do not generally equate a mutable `latest` launcher with a pinned artifact or
ignore extra fields. Once the user saves, normal shared materialization writes
the current pinned definition to every assigned native profile.

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
