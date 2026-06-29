# MCP Crate — Agent Guidelines

The `mcp` crate is the Vibe Kanban MCP (Model Context Protocol) server. It exposes
Vibe Kanban's organisations, projects, issues, workspaces, repositories, and
coding-agent sessions as MCP tools, so external MCP clients (Claude Code, Claude
Desktop, Cursor, Raycast, etc.) can drive Vibe Kanban programmatically.

> See also: [root AGENTS.md](../../AGENTS.md) for repo-wide conventions, and
> [`crates/remote/AGENTS.md`](../remote/AGENTS.md) for the hosted Cloud server.

## Architecture

```
MCP client (Claude Code / Desktop / Cursor / …)
  └── spawns subprocess:  npx -y vibe-kanban@latest --mcp
        └── vibe-kanban-mcp  (this crate, stdio transport)
              └── HTTP → local Vibe Kanban backend  (/api/*)
```

The MCP server is a **stdio** process. It speaks JSON-RPC over stdin/stdout to the
client that launched it, and translates tool calls into HTTP requests against a
**locally running Vibe Kanban backend**. It does not listen on a port and is not
reachable over the network.

Key source:

- `src/bin/vibe_kanban_mcp.rs` — binary entrypoint, arg parsing, backend
  resolution, transport bootstrap (`.serve(stdio())` at `:42`).
- `src/task_server/mod.rs` — `McpServer` struct, `new_global` / `new_orchestrator`
  constructors (`:59-77`), `init()` startup context fetch (`:87-99`).
- `src/task_server/handler.rs` — `ServerHandler` impl (`get_info`, instructions).
- `src/task_server/tools/` — the tool implementations.

## Launch modes

`vibe-kanban-mcp --mode <global|orchestrator>` (default: `global`).

| Mode | Tools | Context |
|------|-------|---------|
| `global` | All tools | None required; backend trusts the local caller. |
| `orchestrator` | Subset scoped to the active workspace + orchestrator session | Resolved from the launching process's `cwd` and the backend port file. |

`global` is what external clients use. `orchestrator` is used by coding agents
running *inside* a Vibe Kanban workspace, where the working directory identifies
the workspace (`fetch_context_at_startup`, `mod.rs:105-120`). In `global` mode a
missing/failed context fetch is non-fatal — `init()` simply drops the
`get_context` tool.

## Backend resolution (important)

`resolve_base_url` (`src/bin/vibe_kanban_mcp.rs:100-134`) decides which backend the
MCP talks to, in this priority order:

1. **`VIBE_BACKEND_URL`** — full URL, used verbatim. **Preferred.**
2. **`MCP_HOST`/`MCP_PORT`** (falling back to `HOST` / `BACKEND_PORT` / `PORT`) —
   assembled into `http://host:port`.
3. **Port file** — `${TMPDIR:-/tmp}/vibe-kanban/vibe-kanban.port`, written by the
   running backend. Read via `read_port_file` (`crates/utils/src/port_file.rs`).

The URL is resolved **once at process start** and cached for the life of the
subprocess (one MCP client session).

### Failure mode: intermittent availability via the port file

When neither `VIBE_BACKEND_URL` nor the port env vars are set, the MCP depends on
the port file. That path is best-effort and has three sharp edges:

- **Read-once, no retry.** `read_port_file` does `fs::read_to_string(path)?` and
  fails on the first miss. The error propagates out of `resolve_base_url`, so the
  binary exits **before** `serve(stdio())` — the MCP server never connects for that
  session.
- **Non-atomic write.** The backend writes the port file in place
  (`fs::write`, no temp+rename). During a backend restart there is a sub-second
  window where the file is empty or partial; an MCP launched in that window reads
  garbage and exits.
- **No staleness check, cached URL.** The reader never verifies the port is
  listening, and the resolved URL is cached for the whole session. If the backend
  later restarts on a different port, the long-lived MCP keeps pointing at the dead
  one — the server stays "connected" but every tool call fails until the client
  session is restarted.

Together these present as **"the MCP is only available intermittently"**, correlated
with backend restarts rather than anything in the client config.

### Fix: pin the backend URL

Set `VIBE_BACKEND_URL` in the MCP server entry's `env` so resolution is
deterministic and never touches the port file. This is durable only if the backend
listens on a fixed port (e.g. `PORT`/`BACKEND_PORT` pinned in `.env` or the service
definition).

```json
{
  "mcpServers": {
    "vibe_kanban": {
      "command": "npx",
      "args": ["-y", "vibe-kanban@latest", "--mcp"],
      "env": { "VIBE_BACKEND_URL": "http://127.0.0.1:3334" }
    }
  }
}
```

> **Caveat:** Vibe Kanban's "Settings → MCP Servers → Save Settings" rewrites this
> entry from `crates/executors/default_mcp.json`, which has no `env` block, so a UI
> save strips `VIBE_BACKEND_URL`. Re-pin after saving, or export
> `VIBE_BACKEND_URL` in the environment that launches the agent so it is inherited
> regardless of the config file.

If fixed ports are not acceptable, the durable code fix is to make resolution
robust: retry-with-backoff in `read_port_file`, atomic (temp+rename) port-file
writes, and reconnect-on-failure in the MCP instead of caching the URL. Those live
in `crates/utils/src/port_file.rs` and this crate.

## How config reaches Vibe Kanban-launched agents

When Vibe Kanban launches a coding agent, it writes MCP config into **that agent's
own global config file** — not a per-session temp file. The canonical default lives
in `crates/executors/default_mcp.json` (embedded at build time as
`PRECONFIGURED_MCP_SERVERS`, `crates/executors/src/mcp_config.rs:25-27`).

Per-executor destinations (`crates/executors/src/executors/mod.rs:127-171`):

| Executor | Config file | MCP key | Format |
|----------|-------------|---------|--------|
| Claude Code | `~/.claude.json` | `mcpServers` | JSON |
| Cursor | `~/.cursor/mcp.json` | `mcpServers` | JSON |
| Codex | `~/.codex/config.toml` | `mcp_servers` | TOML |
| Amp | `~/.config/amp/settings.json` | `amp.mcpServers` | JSON |
| Gemini | `~/.gemini/settings.json` | `mcpServers` | JSON |
| Opencode | `~/.config/opencode/opencode.json[c]` | `mcp` | JSON/JSONC |
| Copilot | `~/.copilot/mcp-config.json` | `mcpServers` | JSON |
| Droid | `~/.factory/mcp.json` | `mcpServers` | JSON |
| Qwen | `~/.qwen/settings.json` | `mcpServers` | JSON |

The canonical stdio definition is adapted to each executor's schema on save by
`apply_adapter` (`crates/executors/src/mcp_config.rs:379-413`). The settings UI
(`packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`)
reads/writes via `GET`/`POST /api/mcp-config`
(`crates/server/src/routes/config.rs:300-407`), which merges into the agent's file
and preserves JSONC comments / TOML structure.

**Most common "my agent doesn't see the MCP" cause:** the MCP Servers page is
**per-executor**. Config saved while one executor is selected lands in that
executor's file only. If your workspaces launch a different executor, switch the
dropdown to match and re-save. Verify with:

```bash
grep -l "vibe.kanban" ~/.claude.json ~/.cursor/mcp.json ~/.codex/config.toml \
  ~/.config/amp/settings.json ~/.gemini/settings.json \
  ~/.config/opencode/opencode.json* ~/.copilot/mcp-config.json \
  ~/.factory/mcp.json ~/.qwen/settings.json 2>/dev/null
```

Note that `~` is the agent's `$HOME`. If sessions run in a sandbox with a different
`$HOME`, the config files above will not exist there.

## Configuring an external client manually

Any MCP client can launch the stdio server directly:

```json
{
  "mcpServers": {
    "vibe_kanban": {
      "command": "npx",
      "args": ["-y", "vibe-kanban@latest", "--mcp"]
    }
  }
}
```

Arguments after `--mcp` are passed through to the `vibe-kanban-mcp` binary
(e.g. `--mode orchestrator`). Add an `env` block with `VIBE_BACKEND_URL` to pin the
backend as described above.

## Remote access (claude.ai) — proposed, not implemented

The stdio server is **local-only**: it cannot be reached over a public URL, so
claude.ai (web) cannot connect to it as-is. claude.ai's remote-MCP support requires
**Streamable HTTP** transport plus **OAuth 2.1 with dynamic client registration
(RFC 7591) and PKCE** — neither of which this crate currently provides
(`rmcp` is built here with `["server", "transport-io"]` only).

The current design sketch for closing that gap (no code yet):

1. Add `transport-streamable-http-server` to `rmcp` and a second binary that serves
   the existing `McpServer` over HTTP, retargeting tool calls from the local
   `/api/*` to the hosted backend's `/v1/*`.
2. Run it as a sidecar with **no public IP**, reachable only through a Cloudflare
   Tunnel (`cloudflared`).
3. Front it with a Cloudflare Worker (`@cloudflare/workers-oauth-provider`) that
   terminates claude.ai's OAuth + DCR, gated at the edge by a Cloudflare Access
   Service Token, and forwards a short-lived inner JWT to the origin.
4. Validate that JWT at the origin into the same `RequestContext` the remote crate
   already produces (`crates/remote/src/auth/middleware.rs`), so authorisation stays
   at the existing backend boundary.

The stdio path described above is unaffected by this work and remains the supported
route for locally-running agents.

## Testing

```bash
cargo test -p mcp
```

Unit tests live alongside the code (e.g. arg parsing in
`src/bin/vibe_kanban_mcp.rs`). Add tests for new tools and for backend-resolution
edge cases.
