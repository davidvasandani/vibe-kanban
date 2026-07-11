# Project knowledge base

Distilled, reusable knowledge from completed tasks. One topic per page; each page lists the
task ids that contributed to it. Consult this index before planning a new task; add or
update pages (and this index) when a task ships something reusable.

| Page | Summary | Contributing tasks |
| --- | --- | --- |
| [claude-log-normalization](claude-log-normalization.md) | How `ClaudeLogProcessor` turns stream-JSON into `/entries/{i}` patches; `EntryIndexProvider` idioms; the AmpResume index-reset gotcha | `4095-thinking-tokens` |
| [collapsing-repeated-log-entries](collapsing-repeated-log-entries.md) | Server-side pattern for collapsing uninterrupted repeated log events into one entry with a `✓` per repeat | `4095-thinking-tokens` |
| [mcp-connectivity-testing](mcp-connectivity-testing.md) | Why VK is an MCP config-writer not a client; the hand-rolled JSON-RPC probe (`crates/executors/src/mcp_test.rs`) covering stdio/streamable-HTTP/legacy-SSE, the `auth_required` (401/403 + `WWW-Authenticate`) classification, the `POST /api/mcp-config/test` route, transport-normalization gotchas, and the duplex-mock test pattern | `6286-mcp-status-and-t`, `0c92-mcp-test-connect` |
| [mcp-oauth-connect](mcp-oauth-connect.md) | The Connect flow for auth-required MCP servers: hand-rolled OAuth client (`mcp_oauth.rs` — RFC 9728/8414 discovery incl. the Keycloak path-issuer gotcha, DCR, PKCE), the `/api/mcp-auth/*` routes with one-shot in-memory flow state, redirect-URI derivation (Host + X-Forwarded-*), popup/autoclose/Save-wipes-token frontend gotchas, and the stub-E2E test recipe | `0c92-mcp-test-connect` |
