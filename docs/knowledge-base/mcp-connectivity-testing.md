# MCP connectivity testing

Tags: `6286-mcp-status-and-t`, `0c92-mcp-test-connect`, `9453-vk-mcp-auto-debu`, `044f-debug-vk-mcp-err`

## The key architectural fact

Vibe Kanban is an **MCP config writer, not an MCP client**. The MCP Servers
settings screen writes server entries into each coding agent's own config file
(`~/.claude.json` `mcpServers`, Codex `mcp_servers` TOML, Opencode `mcp`, …) via
`GET`/`POST /api/mcp-config` (`crates/server/src/routes/config.rs` →
`crates/executors/src/mcp_config.rs`). Nothing in VK ever *connected* to those
servers, so a broken server (wrong command, unreachable URL, expired token) was
only discovered mid-task. The "MCP Status and Test" feature added the first MCP
**client** in the codebase to probe connectivity on demand.

## Where the probe lives

`crates/executors/src/mcp_test.rs` — a self-contained, dependency-free JSON-RPC
probe client. Public surface:

- `test_mcp_servers(servers: HashMap<String, Value>, per_server_timeout) -> Vec<McpServerTestResult>`
  — normalizes each entry, probes concurrently (`futures::future::join_all`),
  each probe wrapped in `tokio::time::timeout`, results sorted by name.
- Types `McpServerTestResult` / `McpServerTestStatus { Ok, Failed, AuthRequired, Unsupported }`
  are ts-rs-exported (registered in `crates/server/src/bin/generate_types.rs`).
  HTTP/SSE probes rejected with 401/403, or with a challenged 3xx redirect,
  classify as `auth_required` (not `failed`) and carry the raw
  `WWW-Authenticate` header in
  `McpServerTestResult.www_authenticate` — the UI's Connect flow feeds it to
  OAuth discovery (see [mcp-oauth-connect](mcp-oauth-connect.md)). The
  classification lives in one choke point, `http_status_error()`; stdio and
  timeout failures stay `failed`.
- Route: `POST /api/mcp-config/test?executor=<agent>` with optional
  `{ servers?: string[] }`; read-only (reuses the same on-disk read path as
  `get_mcp_servers`, never writes).

Frontend: `mcpServersApi.test` / `machineClient.testMcpServers`. The settings
screen (`McpSettingsSection.tsx`) is a form-based per-server-card list, so the
test feature is a header **"Test connection"** button plus a per-card status
icon (✓ connected / ✕ failed / – can't-test) whose hover title carries the tool
count, latency, and server name/version. Results are indexed by server name (the
`servers` map key) and cleared on profile switch and after save so a stale status
never contradicts the on-disk config. An in-flight test is discarded if the user
switches agents before it resolves (a `useRef` guard).

## Why hand-rolled instead of rmcp

`rmcp` (in the workspace for VK's *own* MCP server, `crates/mcp`) only ships
**stdio** and **streamable-HTTP** client transports as of 1.3 — it dropped the
**legacy SSE** client. The user's real servers are legacy SSE
(`http://127.0.0.1:3334/sse`). So the probe is a minimal JSON-RPC client built on
crates already present (`reqwest` json+stream+rustls, `tokio` full,
`serde_json`, `eventsource-stream`) — no new dependency, and it covers all three
transports. The handshake is just `initialize` → `notifications/initialized` →
`tools/list`.

## Transport gotchas worth remembering

- **Server entries are untyped `serde_json::Value`** and are *adapted per agent*
  in `mcp_config.rs` (Gemini renames `url`→`httpUrl`; Codex stdio-only TOML;
  Opencode `type: remote`/`local` with `command` as an array). `normalize()`
  must tolerate all of these and emit `Unsupported { reason }` for anything it
  can't classify — surface that as a distinct status, never a false "failed".
- **stdio** = newline-delimited JSON-RPC on stdin/stdout. Spawn with
  `kill_on_drop(true)` so a timeout (dropped future) kills the child; drain
  stderr in a background task to avoid a full-pipe deadlock and to attach
  diagnostics to errors.
- **Streamable HTTP** = one endpoint; `POST` with
  `Accept: application/json, text/event-stream`. The response is *either* a JSON
  body *or* an SSE stream (`eventsource-stream`); branch on `Content-Type`. Echo
  any `Mcp-Session-Id` response header on the follow-up requests.
- **Do not follow redirects in connectivity probes.** Authentication proxies
  commonly redirect an MCP request to an interactive HTML login page. Observe
  the 3xx directly: a non-empty `WWW-Authenticate` challenge means
  `auth_required`; an unchallenged redirect remains `failed`. A displayed
  `Location` must be reduced to scheme/host/port/path, omitting userinfo, query,
  fragment, and unparseable values.
- **Treat diagnostic bodies as untrusted secret-bearing data.** Cap non-HTML
  previews at 200 displayed characters and redact configured header values
  longest-first so overlapping secrets cannot leak suffixes. Omit HTML-like
  bodies completely because login pages can contain server-generated state or
  CSRF values that cannot be derived from request headers.
- **Legacy SSE** = two channels: `GET` the SSE endpoint, read the `endpoint`
  event to learn the message-POST URL (resolve it with `reqwest::Url::join`),
  then `POST` requests there and read their responses back off the *open GET
  stream*, correlating by JSON-RPC id. Validate the initial GET's media type
  case-insensitively. For message POST acknowledgements, preserve compatibility
  with empty or non-empty plain-text 2xx acknowledgements; reject only clearly
  HTML-like successful bodies with an immediate safe diagnostic.
- The probe spawns the exact command the agent would run — no new security
  surface (user's own machine + config), but do bound everything by a timeout.

## Testing pattern that worked

Extract the stdio handshake into `mcp_handshake_over_io(writer, reader)` generic
over the IO so it can be driven against an in-memory `tokio::io::duplex` mock
server — deterministic, no external process. Unit-test `normalize()` for each
agent shape and the response parsers; test the bogus-command path for `Failed`.

## Failed-diagnostic UI and Debug issue creation

Task `9453-vk-mcp-auto-debu` established the frontend contract for failed saved
assignment tests:

- Treat the backend diagnostic as opaque text. Render it with preserved line
  breaks and long-word wrapping, and pass the exact same string to clipboard
  and issue creation. Use localized fallback text only when the diagnostic is
  absent or empty.
- Embed diagnostics in generated Markdown with a backtick fence longer than
  any backtick run in the diagnostic. This keeps arbitrary logs intact without
  allowing their own fence markers to escape the diagnostic block.
- Settings dialogs are rendered by `NiceModalProvider` as siblings of route
  content, so they do not inherit a `ProjectProvider` mounted inside the active
  project route. A project-aware settings section must read the active
  `projectId` from router params and establish its own route-scoped
  `ProjectProvider`; otherwise optional project context is always absent in the
  modal even on a project page.
- Issue creation uses the existing optimistic `insertIssue` mutation, the first
  status by `sort_order`, and `min(existing sort_order) - 1` for top-of-column
  placement. Await `persisted` before showing success or enabling the explicit
  Open Issue action.
- A React state flag alone is not a sufficient duplicate-submit guard. Retests,
  saves, reloads, and modal unmount/reopen can reset component state while the
  persistence promise is pending. Keep an in-flight guard outside component
  instances, keyed by project plus assignment, acquire it synchronously before
  insertion, and release it only in the persistence path's `finally` block.
