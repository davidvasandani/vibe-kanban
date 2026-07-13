# MCP OAuth Connect flow

Tags: `0c92-mcp-test-connect`

## What it is

When the MCP probe (see [mcp-connectivity-testing](mcp-connectivity-testing.md))
classifies a server as `auth_required` (HTTP 401/403), the settings screen
offers a **Connect** button that runs the full MCP authorization flow
(2025-06-18 spec) from the local backend and writes the obtained access token
into the agent's own config file as `headers.Authorization` — the same place a
user would paste one by hand. No new secret store, no DB change.

## Architecture

- **OAuth client plumbing**: `crates/executors/src/mcp_oauth.rs` — pure
  helpers over `reqwest` (discovery, RFC 7591 DCR, PKCE S256, authorize-URL
  assembly, code exchange). Hand-rolled: the `oauth2` crate doesn't cover the
  MCP-specific discovery steps (RFC 9728 / 8414 / 8707), and the workspace
  already has `sha2`/`rand`/`base64`.
- **Flow routes**: `crates/server/src/routes/mcp_auth.rs` —
  `POST /api/mcp-auth/start?executor=`, `GET /api/mcp-auth/callback`,
  `POST /api/mcp-auth/complete`, `GET /api/mcp-auth/status`. Mounted in
  `relay_signed_routes`; the origin middleware passes header-less top-level
  navigations and the signature middleware passes non-relay requests, so a
  browser redirect reaches the callback (precedent: `handoff_complete` in
  `routes/oauth.rs`). `callback` and `complete` share one `exchange_and_store`
  helper (code exchange + token persist) so the browser and manual paths can't
  drift.
- **Flow state**: module-local `LazyLock<RwLock<HashMap<Uuid, PendingFlow>>>`,
  10-min TTL pruned on access. The exchange inputs (PKCE verifier, client_id,
  token endpoint) live in an `Option` and are `take()`n under the write lock —
  a `state` value can be redeemed exactly once. Tokens are never stored in the
  map, never logged, never in JSON responses.
- **Frontend**: `McpSettingsSection.tsx` — Connect opens a popup, polls
  status at 1s, then re-fetches the config and re-tests just that server via
  the existing `{ servers: [name] }` subset body.

## Gotchas worth remembering (each cost a review round or a debug)

- **Discovery ladder**: `WWW-Authenticate` `resource_metadata` (use the
  probe-captured header as a hint — some servers only challenge on the
  JSON-RPC POST, a plain GET won't re-elicit it) → well-known
  path-insertion + root for `oauth-protected-resource` → PRM
  `authorization_servers[0]` → AS metadata. For AS metadata try RFC 8414
  insertion forms **and the OIDC issuer-suffix form** — path-based issuers
  (Keycloak `…/realms/x`) publish only
  `{issuer}/.well-known/openid-configuration`, which insertion misses.
- **Redirect URI** is derived server-side from the request's `Host` **and
  `X-Forwarded-Proto`/`X-Forwarded-Host`** (HTTPS reverse-proxy setups à la
  `Caddyfile.example` would otherwise register an `http://` URI the AS
  rejects). Relay-proxied requests get a loud error instead — their Host is
  a loopback the user's browser can never reach.
- **Strict-allowlist servers + the loopback escape hatch.** Some
  authorization servers (e.g. `sgsc-mcp`) only accept redirect URIs belonging
  to known MCP clients (Claude/ChatGPT/Codex/Cursor) or `localhost` — a
  server-hosted VK's public callback is rejected at DCR
  ("redirect_uri must be a trusted … callback"). The `loopback: bool` on
  `start` registers `http://localhost:<vk-port>/api/mcp-auth/callback`
  instead (port from `deployment.client_info().get_server_addr()`) and skips
  the relay guard. When the browser can reach that loopback (same machine /
  SSH port-forward) the callback auto-completes; otherwise the user pastes the
  redirected URL — or the bare code — into `/mcp-auth/complete`, which
  `parse_pasted_code`s it, enforces the `state` CSRF binding when the paste
  carried one, consumes the flow's exchange inputs once, and runs
  `exchange_and_store`. The frontend reveals the paste field for loopback
  flows **and** background-polls `status` so the reachable case still
  finalizes (refresh snapshot + re-test) rather than getting stuck in manual
  mode — otherwise a later Save would drop the just-written token.
- **Popup blockers**: `window.open('about:blank', …)` must happen
  synchronously in the click handler; navigate it after the async start call
  resolves. Opening after an `await` loses the transient user activation.
- **Poll-vs-autoclose race**: the success page closes its own window; a
  "popup closed" poll result must do one final status fetch before declaring
  the flow abandoned.
- **Save-wipes-token**: the callback writes the token to disk behind the
  UI's back. On completion the frontend merges the fresh on-disk entry into
  **both** `servers` state and `originalSnapshot`, so a later Save posts the
  header instead of erasing it (and other unsaved edits survive).
- **XSS in result pages**: OAuth `error`/`error_description` params and AS
  error bodies are attacker-influenced and get interpolated into the HTML
  result page — escape them (`html_escape` in `mcp_auth.rs`). The JSON
  status endpoint carries the raw text.
- **This reqwest build has no `.form()`** — encode
  `application/x-www-form-urlencoded` bodies with `Url::query_pairs_mut()`.
- **Bound every outbound call**: a 10s per-request client timeout
  (`oauth_http_client`), same discipline as `MCP_TEST_TIMEOUT`.

## Testing pattern

Unit: pure parts with no I/O (PKCE against RFC 7636 appendix B vector,
header parsing, candidate-URL construction); network parts against one-shot
`tokio::net::TcpListener` stubs writing canned HTTP/1.1 bytes (no
axum/hyper test dep in `executors`). E2E: a ~100-line python stub plays MCP
server + AS (401 challenge, PRM, AS metadata, DCR, token endpoint, MCP
handshake once authorized); drive the real server binary with curl through
test → start → callback → status → re-test, asserting the token lands in
`~/.claude.json` and the replayed callback gets 400. Overriding
`BACKEND_PORT`/`PORT`/`PREVIEW_PROXY_PORT` is required — the dev env exports
the production instance's ports.
