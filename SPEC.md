# SPEC: ChatGPT custom-connector (OAuth) access to the Vibe Kanban MCP

Task: `vk/50df-chatgpt-custom-m`. Feature spec, plan and tasks:
`specs/vk/50df-chatgpt-custom-m/`.

## Problem

Adding `https://vibe.vasandani.dev/mcp` as a ChatGPT custom connector fails
with "Error creating connector". The endpoint exists (supergateway, Streamable
HTTP), but it is locked twice:

1. Cloudflare Access: without a service token or an SSO session the edge
   answers `302` to the Access login page
   (`www-authenticate: Cloudflare-Access …`).
2. Origin (Caddy `:3343` on think2): a static `Authorization: Bearer` token,
   or a `Cf-Access-Jwt-Assertion` header, is required.

ChatGPT connectors support only **No Auth** or **OAuth** (MCP authorization
spec: RFC 9728 protected-resource metadata, RFC 8414 AS metadata, RFC 7591
dynamic client registration, OAuth 2.1 authorization code + PKCE S256). It
cannot send a static bearer or Access service-token headers, so it can never
connect today.

## Decisions (confirmed with the user)

- Change both repos: Vibe Kanban (OAuth server) and homelab (edge + Caddy).
- Open the ChatGPT path past Cloudflare Access, restricted to OpenAI's
  published connector egress ranges (`openai.com/chatgpt-connectors.json`),
  with OAuth enforced at the origin.
- ChatGPT gets the full global tool set.

## Design

### Separate public path: `/oauth/mcp`

The existing `/mcp` route trusts the *presence* of `Cf-Access-Jwt-Assertion`.
That is safe only while every `/mcp` request passes Access. An IP-allowlisted
Access bypass on `/mcp` would let any request from OpenAI egress (e.g. a GPT
Action with a custom header) forge that header. Instead ChatGPT gets its own
resource, `https://vibe.vasandani.dev/oauth/mcp`, which accepts **only** a
Vibe Kanban-issued OAuth access token. `/mcp` and its clients are unchanged.

### Vibe Kanban: embedded OAuth 2.1 authorization server

Opt-in: enabled only when `VK_MCP_OAUTH_PUBLIC_URL` (e.g.
`https://vibe.vasandani.dev`) is set to an absolute `https` URL (or `http`
loopback for development). Unset, every new route answers `404`.

Issuer = `VK_MCP_OAUTH_PUBLIC_URL`; resource = `<issuer>/oauth/mcp`.

| Route | Edge | Purpose |
|---|---|---|
| `GET /.well-known/oauth-protected-resource[/oauth/mcp]` | bypass (OpenAI IPs) | RFC 9728 metadata: `resource`, `authorization_servers`, `scopes_supported=["mcp"]`, `bearer_methods_supported=["header"]` |
| `GET /.well-known/oauth-authorization-server[/…]` | bypass (OpenAI IPs) | RFC 8414 metadata (`code` only, `authorization_code`+`refresh_token`, `S256` only, auth methods `none`/`client_secret_post`/`client_secret_basic`, `registration_endpoint`) |
| `POST /oauth/register` | bypass (OpenAI IPs) | RFC 7591 DCR |
| `GET/POST /oauth/authorize` | **Access SSO** | Server-rendered consent page; approve/deny |
| `POST /oauth/token` | bypass (OpenAI IPs) | code exchange + refresh-token rotation |
| `GET /api/mcp-oauth/verify` | (loopback, Caddy `forward_auth`) | `200` for a valid access token, else `401` with `WWW-Authenticate: Bearer resource_metadata="…"` and a JSON-RPC error body |
| `GET /api/mcp-oauth/grants`, `DELETE /api/mcp-oauth/grants/{id}` | Access SSO | list / revoke authorized connectors |

Rules:

- **Registration:** `redirect_uris` required. Each must be `https` (or `http`
  loopback), with no fragment. Only `authorization_code`/`refresh_token` and
  `code`. `client_secret_post`/`basic` registrations get a secret (only its
  hash is stored); `none` gets no secret. Body ≤ 16 KiB. Clients that never
  completed an authorization are pruned after 24 h, and the number of clients
  is capped (fails with `400 invalid_client_metadata` once full).
- **Authorize:** `client_id` and an exact registered `redirect_uri` are
  validated *before* anything redirects (failure → HTML error page, never a
  redirect). After that, protocol errors redirect back with `error` and
  `state`. Requires `response_type=code` and `code_challenge` with
  `code_challenge_method=S256`. `scope` ⊆ {`mcp`} (default `mcp`). `resource`,
  when present, must equal the resource URL. GET stores a pending request
  with a random one-time consent token (hash stored) and renders the form.
  POST requires the matching consent token and an unexpired pending request
  (10 min). Approve issues a single-use code (60 s, hash stored), redirects
  with `code`, `state`, `iss`. Deny redirects with `access_denied`. The
  responses carry `Cache-Control: no-store`, `X-Frame-Options: DENY`,
  `Content-Security-Policy: frame-ancestors 'none'` and
  `Referrer-Policy: no-referrer`.
- **Token:** client authentication matches the registered method
  (constant-time). Code exchange verifies the client, redirect_uri, PKCE
  (`BASE64URL(SHA256(verifier))`, verifier 43–128 unreserved characters), the
  single-use code and its expiry. Replaying a used code revokes the grant it
  minted. Each successful exchange creates a grant, a 1 h access token and a
  30 d refresh token (opaque 256-bit random values, SHA-256 hashes stored).
  Refresh rotates: the old refresh token is revoked. Reusing a revoked
  refresh token revokes the whole grant. Errors are RFC 6749 JSON
  (`invalid_grant`, `invalid_client` with `401`, …) with `no-store`.
- **Verify:** reads `Authorization: Bearer`, hashes it, and accepts it only
  if it is an unexpired, unrevoked access token whose resource is this
  resource. It updates `last_used_at` at most once a minute.
- No new secrets in config files; plaintext tokens never logged or stored.

Persistence (SQLite migration): `mcp_oauth_clients`,
`mcp_oauth_authorizations` (pending request + code), `mcp_oauth_grants`, and
`mcp_oauth_tokens`. Expired rows are pruned opportunistically.

### Homelab

1. `terragrunt/modules/cloudflare-tunnel`: new per-hostname
   `bypass_access_ip_lists` (path → name of a Zero Trust IP list), backed by a
   `cloudflare_zero_trust_list` (type `IP`) so 278 CIDRs aren't inlined into
   policies. `bypass_access_ip_ranges` keeps working unchanged.
2. `cloudflare-tunnel-vibe-remote`: bypass `/oauth/mcp`, `/oauth/register`,
   `/oauth/token`, `/.well-known/oauth-protected-resource`,
   `/.well-known/oauth-authorization-server` for OpenAI connector ranges,
   vendored as `openai-chatgpt-connectors.json` plus a refresh script.
   `/oauth/authorize` and `/mcp` stay on hostname Access.
3. think2 Caddy `:3343`: `handle /oauth/mcp*` → `forward_auth
   127.0.0.1:3334 { uri /api/mcp-oauth/verify }`, then rewrite to `/mcp` and
   proxy to supergateway `:8787`. It never consults the static bearer or
   `Cf-Access-Jwt-Assertion`. All other `/oauth/*` and `/.well-known/*` fall
   through to the VK server (`:3334`).
4. `vibe-kanban-dev` gets `VK_MCP_OAUTH_PUBLIC_URL=https://vibe.vasandani.dev`.
5. Docs: `docs/vibe-kanban-mcp-deployment.md` ChatGPT section.

## Non-goals

- Changing `/mcp` auth or existing clients.
- A Settings UI for grants (API only; UI can follow).
- Per-tool scopes (user chose all tools).
- Public `/oauth/revoke` (revocation goes through the SSO-gated management API).

## Acceptance

- Unit/route tests cover metadata, DCR validation, the authorize
  error/redirect split, consent-token checks, PKCE, code single use and
  replay, refresh rotation and reuse revocation, verify accept/reject, and
  disabled-by-default `404`.
- `cargo test -p server`, `cargo clippy`, `pnpm run format` clean. Homelab
  Nix/terraform validation passes in CI.
- After deploy, a full end-to-end ChatGPT run:
  `https://vibe.vasandani.dev/oauth/mcp`, Authentication = OAuth, connector
  creation succeeds and tools list.
