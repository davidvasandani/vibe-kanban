# Technical Spec: MCP Test connection — surface auth failures + Connect (OAuth)

> Task 0c92. Full SpecKit artifacts live in
> `homelab/specs/vk/0c92-mcp-test-connect/` (`spec.md`, `plan.md`,
> `research.md`, `data-model.md`, `contracts/`, `tasks.md`). This file is the
> repo-root technical summary.

## Problem

The MCP "Test connection" feature (PR #77) shows an OAuth-protected server
(e.g. `sgsc-mcp`) as a red ✕ indistinguishable from a server that is down, the
error text hides in a hover `title` tooltip (invisible on touch), and there is
no way to authenticate the server from Vibe Kanban — the user must run an
interactive `claude mcp` / `/mcp` session elsewhere.

## Solution

### 1. Probe classification (`crates/executors/src/mcp_test.rs`)

`McpServerTestStatus` gains `AuthRequired` (`"auth_required"`). An internal
`ProbeError { AuthRequired, Other }` threads through the http/sse probe paths;
`http_status_error()` classifies HTTP 401/403 and captures the
`WWW-Authenticate` header into a new `McpServerTestResult.www_authenticate:
Option<String>` field. Stdio probes and non-auth failures (timeouts,
connection refused, other HTTP statuses) are unchanged (`failed` /
`unsupported`).

### 2. OAuth client plumbing (`crates/executors/src/mcp_oauth.rs`, new)

Hand-rolled over existing workspace deps (`reqwest`, `sha2`, `rand`,
`base64` — no new dependency), implementing the MCP authorization spec
(2025-06-18):

- `discover()` — protected-resource metadata via `WWW-Authenticate`
  `resource_metadata` (RFC 9728) with `/.well-known/oauth-protected-resource`
  path-insertion fallbacks, then AS metadata (RFC 8414 → OIDC discovery
  fallback); falls back to the MCP origin as AS for pre-9728 servers.
- `register_client()` — RFC 7591 dynamic client registration as a public
  client (`token_endpoint_auth_method: "none"`).
- `Pkce::generate()` (S256, verified against RFC 7636 appendix B),
  `build_authorize_url()` (includes RFC 8707 `resource`), `exchange_code()`
  (returns only the access-token string).

### 3. Flow endpoints (`crates/server/src/routes/mcp_auth.rs`, new)

Mounted in `relay_signed_routes` (same middleware group as `/mcp-config/*`
and the existing `handoff_complete` browser-redirect callback):

- `POST /api/mcp-auth/start?executor=` `{ server_name }` — resolves the
  agent's on-disk config entry, requires a URL transport, runs discovery +
  DCR, builds the redirect URI from the request's `Host` header
  (`http://{host}/api/mcp-auth/callback` — same-origin with the frontend, so
  auto-assigned dev ports need no config), stores a pending flow, returns
  `{ flow_id, authorize_url }`.
- `GET /api/mcp-auth/callback?code&state` — looks up the flow by the random
  `state` (CSRF binding), **consumes the exchange inputs atomically** (a
  state can't be redeemed twice), exchanges the code, writes
  `headers.Authorization = "Bearer <token>"` onto the server's config entry
  via the existing `update_mcp_servers_in_config` path, and returns the
  auto-closing HTML success page (reused from `routes/oauth.rs`).
- `GET /api/mcp-auth/status?flow_id` — `pending | completed | failed` (+
  error). Never returns the token.

Pending flows live in a module-local `LazyLock<RwLock<HashMap<Uuid,
PendingFlow>>>` with a 10-minute TTL, pruned on access. Tokens are never
logged, never stored in the flow map, and never returned in JSON.

### 4. Frontend (`packages/web-core`)

- `McpTestStatusIcon`: `auth_required` → Phosphor `LockKeyIcon` +
  `text-warning`, distinct from ✕ `failed`.
- New `McpTestResultDetails` line under each non-ok server card: the error
  text renders inline (clamped to 2 lines, click to expand — readable
  without hover on all pointer types); warning palette for auth-required
  (with an "Authentication required" label), error palette for failures,
  neutral for unsupported.
- **Connect** button on auth-required cards (disabled while dirty, mirroring
  the Test button): `startMcpAuth` → `window.open` consent popup
  (OAuthDialog pattern) → 1s status polling (stops on completed/failed/popup
  closed, with a final re-check to beat the auto-close race) → on completion
  the connected server's fresh on-disk entry is merged into both the working
  state and the pristine snapshot (so a later Save cannot wipe the token)
  and just that server is re-tested via the existing subset-test body.
- `machineClient.startMcpAuth` / `getMcpAuthStatus`; ts-rs types regenerated
  (`McpAuthStartRequest/Response`, `McpAuthFlowState`,
  `McpAuthStatusResponse`); translations added for all 7 locales.

## Verification

- 14 new unit tests (`cargo test -p executors mcp_`): 401/403/500/refused
  classification, `WWW-Authenticate` capture, discovery fallbacks, DCR,
  PKCE vector, authorize-URL assembly, code exchange, replay of existing
  transport-normalization behavior.
- Stub E2E against the real server binary: 401 → `auth_required` with header
  captured → `start` returns a PKCE authorize URL → simulated redirect to
  `callback` exchanges the code → token lands in `~/.claude.json` → re-test
  returns `ok` (1 tool) → replayed callback rejected with 400.
- Gates: `pnpm run generate-types:check`, `pnpm run check`, `pnpm run lint`,
  `cargo test --workspace`, `pnpm run format` — all green.

## Out of scope (v1)

Token refresh/expiry management, stdio-server auth, encrypted secret
storage (parity with hand-pasted headers), remote deployment, mirroring the
token to other agents' configs.
