# Implementation Plan: MCP auth-failure surfacing + Connect (task vk/0c92-mcp-test-connect)

Step-by-step build order. The authoritative dependency-ordered task list is
`homelab/specs/vk/0c92-mcp-test-connect/tasks.md` (T001–T012); this is the
executable narrative. Rationale in `SPEC.md`; prior-art recall in
`PRIOR_KNOWLEDGE.md`.

## Step 1 — Classify 401/403 in the probe (T001–T002)

In `crates/executors/src/mcp_test.rs`: add `AuthRequired` to
`McpServerTestStatus` and `www_authenticate: Option<String>` to
`McpServerTestResult`; introduce private `ProbeError { AuthRequired { www_authenticate, message }, Other }`
with `From<String>` so existing `format!(...)?` sites keep converting; add
`http_status_error(resp)` as the single non-success choke point and call it
from `http_send`, `probe_sse`'s initial GET, and `sse_post`; map the error in
`test_one`/`failed`. Stdio and timeout paths stay `Failed`. Tests: one-shot
`tokio::net::TcpListener` stubs for 401(+header)/403/500, bind-then-drop for
connection-refused, existing unsupported/stdio tests untouched.

## Step 2 — OAuth client plumbing (T003)

New `crates/executors/src/mcp_oauth.rs` (register in `lib.rs`): pure helpers
over `reqwest::Client` — `discover(client, mcp_url, www_authenticate_hint)`
(RFC 9728 `resource_metadata` parse → well-known path-insertion fallbacks →
PRM → RFC 8414/OIDC AS metadata, MCP-origin fallback), `register_client`
(RFC 7591, public client), `Pkce::generate` (S256), `build_authorize_url`
(PKCE + `state` + RFC 8707 `resource` + scopes), `exchange_code` (returns
only the token string; encode the form body via `Url::query_pairs_mut` —
this reqwest build has no `.form()`). Unit-test pure parts without I/O
(PKCE against RFC 7636 appendix B) and network parts against loopback stubs.

## Step 3 — Flow endpoints (T004–T006)

Make `update_mcp_servers_in_config` / `get_mcp_servers_from_config_path`
`pub(crate)` in `routes/config.rs` and `simple_html_response` /
`close_window_response` `pub(crate)` in `routes/oauth.rs`. New
`crates/server/src/routes/mcp_auth.rs` with a module-local
`LazyLock<RwLock<HashMap<Uuid, PendingFlow>>>` (10-min TTL, pruned on
access; `ExchangeInputs` held in an `Option` and `take()`n under the write
lock so a state can't be redeemed twice):

- `POST /mcp-auth/start?executor=` `{ server_name }` — agent → config path →
  server entry URL (`url`/`httpUrl`; stdio → error) → redirect URI from the
  request `Host` header → discover + DCR (missing `registration_endpoint` →
  actionable error naming the authorization endpoint) → store flow → return
  `{ flow_id, authorize_url }`.
- `GET /mcp-auth/callback` — flow by `state`; `error` param or exchange
  failure → mark failed + error page; success → write
  `headers.Authorization = "Bearer <token>"` on the entry through the
  existing config read-modify-write, mark completed, auto-closing page.
- `GET /mcp-auth/status?flow_id` — pending/completed/failed (+ error), never
  the token.

Mount `mcp_auth::router()` next to `config::router()` in
`routes/mod.rs` `relay_signed_routes` (origin middleware passes header-less
top-level navigations; signature middleware passes non-relay requests — the
`handoff_complete` precedent).

## Step 4 — Types (T007)

Register the four new types in `crates/server/src/bin/generate_types.rs`;
`pnpm run generate-types` (never hand-edit `shared/types.ts`).

## Step 5 — Frontend (T008–T010)

`machineClient.ts`: `startMcpAuth(query, serverName)` / `getMcpAuthStatus`
modeled on `testMcpServers`. `McpSettingsSection.tsx`: extend
`McpTestStatusIcon` (`auth_required` → `LockKeyIcon` + `text-warning`); new
`McpTestResultDetails` inline line under each non-ok card (clamped
`line-clamp-2`, click-to-expand button, warning/error/neutral palettes,
"Authentication required" label); card row becomes a column. Connect flow:
start → `window.open` popup → 1s poll loop that stops on non-pending status
or popup close (with a final status re-check to beat the success page's
auto-close race) → on completed, merge the fresh on-disk entry into **both**
`servers` and `originalSnapshot` (so Save can't wipe the token and other
unsaved edits survive) → subset re-test `{ servers: [name] }` merged into
`testResults`. Respect the `activeProfileRef` stale-guard everywhere;
disable Connect while `isDirty` (same rule as Test). Add the six new
`settings.mcp.test.*` strings to all 7 locales.

## Step 6 — Gates + E2E (T011–T012)

`pnpm run generate-types:check` && `pnpm run check` && `pnpm run lint` &&
`cargo test --workspace` && `pnpm run format`. E2E against the real binary:
python stub playing MCP server + OAuth AS (401 with `resource_metadata`,
PRM, AS metadata, DCR, token endpoint, MCP handshake when authorized);
scratch server entry in `~/.claude.json` (backed up); drive
test → start → callback → status → re-test via curl; assert `auth_required`
→ token persisted → `ok` (tool count 1) → replayed callback 400; restore the
config byte-for-byte and kill the processes.
