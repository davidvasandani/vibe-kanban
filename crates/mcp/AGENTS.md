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

Both modes include the background-helper tools (`spawn_background_helper`,
`list_background_helpers`, `stop_background_helper`,
`src/task_server/tools/background_helpers.rs`). These are the sanctioned way for
an agent to keep a long-lived subprocess (watcher, tunnel, log follower) running
past the end of its turn: the backend spawns the helper as a tracked
`BackgroundHelper` execution process in its own process group, so the turn-end
process-group reap cannot hit it, while it stays visible in the Processes tab,
survives server restarts like a dev server, and is stopped on workspace archive.
Agents should use these instead of `setsid`/`nohup` tricks, which leak untracked
orphans.

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
the port file. Historically that path had three sharp edges, each now addressed in
code (see **Resilience** below):

- **Read-once, no retry.** `read_port_file` used to do `fs::read_to_string(path)?`
  and fail on the first miss. The error propagated out of `resolve_base_url`, so the
  binary exited **before** `serve(stdio())` — the MCP server never connected for that
  session. → *Now retried with backoff.*
- **Non-atomic write.** The backend wrote the port file in place
  (`fs::write`, no temp+rename). During a backend restart there was a sub-second
  window where the file was empty or partial; an MCP launched in that window read
  garbage and exited. → *Now written atomically via temp+rename.*
- **No staleness check, cached URL.** The reader never verified the port was
  listening, and the resolved URL was cached for the whole session. If the backend
  later restarted on a different port, the long-lived MCP kept pointing at the dead
  one — the server stayed "connected" but every tool call failed until the client
  session was restarted. → *Now re-resolved and retried on connection failure.*

Together these presented as **"the MCP is only available intermittently"**, correlated
with backend restarts rather than anything in the client config. The classic symptom
of the third edge: the tool **reconnects at the tooling layer, but every backend call
fails** (e.g. "Unable to connect. Is the computer able to access the url?").

### Resilience (implemented)

Backend resolution now self-heals without any config change:

- **Atomic port-file writes** — `write_port_file_with_proxy`
  (`crates/utils/src/port_file.rs`) writes to a process-unique `*.tmp` file and
  `rename`s it into place, so readers never observe an empty/partial file.
- **Retry-with-backoff reads** — `read_port_info` retries the read a bounded number
  of times (~1s total) to ride out the restart window instead of failing on the
  first miss.
- **Reconnect-on-failure** — `McpServer` holds `base_url` behind an
  `Arc<RwLock<String>>`. When a tool's HTTP call fails with a transient connection
  error (`send_with_reconnect` in `src/task_server/tools/mod.rs`), the server
  re-resolves the backend URL (`crate::backend::resolve_base_url`), retargets the
  request at the new host/port, and retries once. A long-lived session therefore
  follows the backend across a restart+port-change instead of staying pinned to the
  dead port. URL changes and retries are logged at `warn`.

### Still preferred: pin the backend URL

Set `VIBE_BACKEND_URL` in the MCP server entry's `env` so resolution is
deterministic and never touches the port file at all. This is durable only if the
backend listens on a fixed port (e.g. `PORT`/`BACKEND_PORT` pinned in `.env` or the
service definition).

For a self-hosted/prod server, the most durable form is to pin both
`BACKEND_PORT` and `VIBE_BACKEND_URL` in the service manager's environment so the
server *and* the coding agents it launches inherit them. See the example
`vibe-kanban.service.example` + `vibe-kanban.env.example` at the repo root
(front the loopback port with a reverse proxy — see `Caddyfile.example`). Those
examples also run **our** build — the compiled `server` binary directly rather
than the public `npx vibe-kanban` package — and set `VIBE_KANBAN_MCP_COMMAND`
(see below) so launched agents spawn our MCP binary too.

### Launching our MCP build in agents (not the public package)

The MCP entry written into launched agents' configs comes from
`crates/executors/default_mcp.json`, whose default is `npx -y vibe-kanban@latest
--mcp` — the **public** package. A self-hosted deployment overrides this without
patching the file via two env vars read by `PRECONFIGURED_MCP_SERVERS`
(`crates/executors/src/mcp_config.rs`):

- `VIBE_KANBAN_MCP_COMMAND` — the executable (e.g. the co-located
  `vibe-kanban-mcp` binary from `local-build.sh`). Running the binary directly
  needs no args (defaults to global mode).
- `VIBE_KANBAN_MCP_ARGS` — optional whitespace-separated args; e.g. to pin a
  privately published package: `command=npx`,
  `args="-y @ourscope/vibe-kanban@X.Y.Z --mcp"`.

Unset ⇒ the public default is used unchanged, so this is opt-in and doesn't
affect normal/public installs.

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

Pinning is still the most deterministic option, but it is no longer *required* for
stability — the resolution path is now robust on its own (retry-with-backoff reads
and atomic writes in `crates/utils/src/port_file.rs`, reconnect-on-failure in this
crate). See **Resilience** above.

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
