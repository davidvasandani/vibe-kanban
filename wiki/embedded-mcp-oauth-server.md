# Embedded OAuth server for remote MCP clients

VK can act as the OAuth 2.1 authorization server for MCP clients that can
only authenticate with OAuth (ChatGPT custom connectors). The code lives in
`crates/server/src/mcp_oauth/` and `crates/db/src/models/mcp_oauth.rs`. It
is opt-in through `VK_MCP_OAUTH_PUBLIC_URL`; every route returns 404 when
that is unset.

## Design decisions worth keeping

- **VK issues and verifies; it does not carry the MCP traffic.** The
  transport stays the deployment's stdio→HTTP bridge. The reverse proxy
  `forward_auth`s `/oauth/mcp` to `/api/mcp-oauth/verify` (`204` or `401`
  with `WWW-Authenticate: Bearer resource_metadata=…` and a JSON-RPC body).
  Serving MCP natively would duplicate a working bridge.
- **Opaque tokens, hashed at rest.** Revocation needs a DB lookup anyway,
  so JWTs would only add key management. Every secret (client secret,
  consent nonce, code, access and refresh token) is stored as SHA-256 and
  compared in constant time.
- **The model takes `now` explicitly.** Expiry, rotation and pruning are
  tested deterministically in `db` without clock mocking. Route tests only
  need the happy path and the protocol errors.
- **Timestamps are fixed-width RFC 3339 micros (`timestamp()`),** so SQLite
  string comparison orders them. Do not mix in `datetime('now')` values in
  these tables.

## Race and replay rules (from the Codex review)

- **Losing a concurrent code exchange counts as a replay.** Both requests
  can read the row while it is `approved`. The conditional `UPDATE … WHERE
  status='approved'` lets one win, and the loser must re-read the row and
  revoke the winner's grant (`revoke_exchanged_grant`). Revoking only on
  sequential replay leaves a leaked code's tokens alive.
- **Check bindings before replay revocation.** Client, redirect URI and
  PKCE verifier must match before an `exchanged` code triggers revocation.
  Otherwise anyone holding a leaked code can register their own client and
  revoke the legitimate grant (a denial of service).
- **Caps must be enforced in the insert,** not by a prior `COUNT`:
  `INSERT … SELECT … WHERE (SELECT COUNT(*) …) < ?`. One SQLite statement is
  atomic, so concurrent registrations cannot overshoot.
- **Refresh rotation uses the same shape.** `UPDATE … WHERE revoked_at IS
  NULL` decides the winner. `AlreadyRevoked` is treated as reuse and revokes
  the grant.

## Protocol details that are easy to get wrong

- Validate `client_id` and the exact registered `redirect_uri` before
  anything can redirect. Failures render an HTML error page. After that,
  errors redirect with `error`, `state` and `iss`.
- RFC 7591's default `token_endpoint_auth_method` is
  `client_secret_basic`. A client that omits the method gets a secret and
  must use it. Echo the method actually assigned.
- Don't put `form-action 'self'` in the consent page's CSP. Chrome applies
  `form-action` to the redirect that follows the POST, which would block
  the hop back to the client's redirect URI.
- Mount the public routes at the top level before the SPA `/{*path}`
  fallback. Nest the API router with `nest_service("/mcp-oauth", …)`,
  because it carries its own state type.

## Contributed by

- `vk/50df-chatgpt-custom-m`
