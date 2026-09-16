# Research: Recover Wedged MCP Sessions

## Decision 1: Generalize the existing restart reservation

The current `/api/sessions/{id}/queue/mcp-restart` route already solves the
hardest self-restart race. It reserves a queued message invisibly, checks running
state twice, commits only after confirmation, and lets the exit monitor claim the
continuation. A drop guard cancels abandoned reservations. The implementation
will move this behavior behind the container service and call it from both HTTP
UI and MCP paths.

Rejected: directly kill the calling executor from the MCP request. That can
destroy the tool response and bypass the sole exit-monitor consumer.

## Decision 2: Claude Code live refresh is unsupported

Only the Codex executor creates an `McpRefreshSignal` and implements the
`config/mcpServer/reload` plus `mcpServerStatus/list` contract. Claude Code's
executor publishes no equivalent control in the current code. Therefore
`refresh_mcp_tools` for `CLAUDE_CODE` is explicitly `unsupported`; the recovery
action is a fresh process via `restart_session`.

Claude Code does emit the fresh process's registered tool-name inventory in its
structured `system/init` event. VK already deserializes the optional `tools`
field but currently discards it during normalization. Recovery status can use
that executor-owned startup evidence without claiming live reload support.

Rejected: configuration rewrite or an independent connectivity test presented
as refresh. Neither proves adoption by the active Claude process.

## Decision 3: Zero tools is terminal but not necessarily erroneous

An MCP server can expose resources or prompts without tools. Successful
discovery with zero tools therefore maps to `connected_no_tools`, not a generic
failure. The UI must say “0 tools registered,” and tool capability remains
unavailable. A server that never completes discovery maps to a named failure.

## Decision 4: One absolute discovery deadline

The observer records `deadline_at` when a recovery generation begins. Polling,
`CONNECT_TIMEOUT`, and `CONNECTION_CLOSED` events never move it. On expiry every
nonterminal server becomes `failed_timeout`. Error observations use a bounded
ring and allow-listed codes, preventing memory growth and secret leakage.

Rejected: reset a timeout on each reconnect. That recreates the indefinite
“still connecting” failure from the report.

## Decision 5: Runtime registry and settings test are separate projections

The settings test is valuable evidence that VK can independently handshake with
a saved definition. It is not evidence about an existing agent process. The UI
will render both when available and derive mismatch only from explicitly named
states. Runtime registry state wins any “tools available” label.

## Decision 6: No new dependency

Tokio, Chrono, Serde, Axum, ts-rs, existing executor protocols, and existing UI
helpers cover coordination, timeouts, data contracts, and issue generation.
