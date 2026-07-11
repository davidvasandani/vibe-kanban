# Prior Knowledge — recalled for `vk/0c92-mcp-test-connect`

Searched both project knowledge bases — `docs/knowledge-base/` (3 pages +
INDEX) and `wiki/` (8 topic pages + INDEX) — for pages relevant to this task
(classifying MCP probe auth failures, surfacing them in the settings UI, and
adding an OAuth "Connect" flow). One page is directly on-topic; two more set
constraints the design must honor.

## Relevant findings

**[docs/knowledge-base/mcp-connectivity-testing.md] — directly on-topic.**
The feature this task extends. Key facts reused wholesale: VK is an MCP
*config-writer*, and the probe in `crates/executors/src/mcp_test.rs` is the
codebase's only MCP client — hand-rolled over `reqwest`/`tokio`/`serde_json`/
`eventsource-stream` because rmcp 1.3 lacks a legacy-SSE client. Server
entries are untyped `serde_json::Value` adapted per agent (Gemini `httpUrl`,
Opencode `type: local` arrays, Codex stdio-only TOML) — `normalize()` must
stay tolerant and `unsupported` must never become a false `failed`. The route
(`POST /api/mcp-config/test?executor=`) already supports subset testing via
`{ servers: [name] }` — this is what makes the post-Connect single-server
re-test free. Testing pattern to follow: IO-generic handshake + in-memory
mocks, no external processes. Direct consequence for this task: classify
401/403 inside the probe's HTTP error path (one choke point,
`http_status_error`), and test with loopback stubs in the same spirit.

**[wiki/external-connector-sync.md] — credential rules that transfer.** Two
rules from the Jira connector shaped the OAuth flow design:
(1) *secrets never appear in API responses, logs, or error messages* — hence
the status endpoint never returns the token, `exchange_code()` returns only
the token string, and only OAuth *error* bodies (RFC 6749 §5.2, no secrets)
are echoed into messages; (2) *the stored-credential destination-pinning
rule*: any endpoint that combines a stored secret with a caller-supplied URL
is an exfiltration primitive. The `/mcp-auth/start` endpoint honors this by
taking only a server *name* in the body — the URL is read from the agent's
on-disk config, never from the request — and the token endpoint the code is
redeemed against comes from the server's own discovered metadata, pinned in
the pending flow at start time, not from the callback request.

**[wiki/self-hosted-deployment.md, wiki/project-context-map.md] — scope
boundary.** The MCP settings screen is the *local* stack
(`packages/web-core` settings dialog + local axum server), not the remote
Postgres/Electric stack — so no migrations, no `crates/remote` involvement,
and per-flow state can be process-local in-memory (matching the existing
`oauth_handoffs` precedent in `crates/local-deployment`).

## Checked and not relevant

`claude-log-normalization`, `collapsing-repeated-log-entries` (log pipeline),
`appbar-rail-and-org-tiles`, `kanban-*`, `mobile-kanban-scrolling` (kanban
UI), `electric-sync-fallback` (remote sync) — different subsystems.
