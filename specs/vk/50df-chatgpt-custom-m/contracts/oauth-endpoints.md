# Contract: VK OAuth endpoints

`ISS` = `VK_MCP_OAUTH_PUBLIC_URL` without a trailing slash (e.g.
`https://vibe.vasandani.dev`). `RES` = `ISS + "/oauth/mcp"`. When unset,
every route below returns `404`.

## GET /.well-known/oauth-protected-resource and /.well-known/oauth-protected-resource/oauth/mcp
200 JSON:
```json
{"resource":"RES","authorization_servers":["ISS"],"scopes_supported":["mcp"],
 "bearer_methods_supported":["header"],"resource_name":"Vibe Kanban"}
```

## GET /.well-known/oauth-authorization-server (also with `/oauth/mcp` suffix)
200 JSON:
```json
{"issuer":"ISS","authorization_endpoint":"ISS/oauth/authorize",
 "token_endpoint":"ISS/oauth/token","registration_endpoint":"ISS/oauth/register",
 "scopes_supported":["mcp"],"response_types_supported":["code"],
 "response_modes_supported":["query"],
 "grant_types_supported":["authorization_code","refresh_token"],
 "token_endpoint_auth_methods_supported":["none","client_secret_post","client_secret_basic"],
 "code_challenge_methods_supported":["S256"],
 "authorization_response_iss_parameter_supported":true}
```

## POST /oauth/register (JSON, ≤ 16 KiB)
Request fields: `redirect_uris` (required), `client_name`,
`token_endpoint_auth_method` (default `client_secret_basic` per RFC 7591),
`grant_types` (default `["authorization_code"]`), `response_types` (default
`["code"]`), and `scope`. Unknown fields are ignored.
201 → `{client_id, client_id_issued_at, client_name?, redirect_uris,
grant_types, response_types, token_endpoint_auth_method, scope:"mcp",
client_secret?, client_secret_expires_at?: 0}`.
400 → `{"error":"invalid_redirect_uri"|"invalid_client_metadata","error_description":"…"}`.

## GET /oauth/authorize
Query: `response_type, client_id, redirect_uri, code_challenge,
code_challenge_method, state?, scope?, resource?`.
- Unknown client, or `redirect_uri` not registered (when exactly one is
  registered, it may be omitted) → 400 HTML error, no redirect.
- Other invalid params → 302 `redirect_uri?error=…&error_description=…&state=…&iss=ISS`
  (`unsupported_response_type`, `invalid_request`, `invalid_scope`,
  `invalid_target`).
- Valid → 200 HTML consent form (POST, fields `request_id`, `consent`,
  `decision=approve|deny`).
Headers on every HTML response: `Cache-Control: no-store`,
`X-Frame-Options: DENY`, `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'`,
`Referrer-Policy: no-referrer`.

## POST /oauth/authorize (form)
- Unknown request, wrong consent token, not pending, or expired → 400 HTML.
- approve → 302 `redirect_uri?code=…&state=…&iss=ISS`.
- deny → 302 `redirect_uri?error=access_denied&state=…&iss=ISS`.

## POST /oauth/token (form)
- `grant_type=authorization_code`: `code, redirect_uri, code_verifier,
  client_id` (+ secret per method).
- `grant_type=refresh_token`: `refresh_token, client_id` (+ secret).
200 → `{"access_token","token_type":"Bearer","expires_in":3600,"refresh_token","scope":"mcp"}`.
Errors: `invalid_request`, `invalid_client` (401), `invalid_grant`,
`unsupported_grant_type`, `invalid_target`. Always sent with
`Cache-Control: no-store` and `Pragma: no-cache`.

## GET|POST|DELETE /api/mcp-oauth/verify (Caddy forward_auth)
- Valid access token → `204`.
- Otherwise → `401` with `WWW-Authenticate: Bearer resource_metadata="ISS/.well-known/oauth-protected-resource/oauth/mcp"`
  (plus `, error="invalid_token"` when a token was presented), and the body
  `{"jsonrpc":"2.0","id":null,"error":{"code":-32001,"message":"Unauthorized: …"}}`.

## GET /api/mcp-oauth/grants
`ApiResponse<[{id, client_id, client_name, created_at, last_used_at}]>`
listing live grants only.

## DELETE /api/mcp-oauth/grants/{id}
Revokes the grant → `ApiResponse<()>`. An unknown id → 404.
