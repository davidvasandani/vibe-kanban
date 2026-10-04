# Research: ChatGPT custom-connector OAuth

## Evidence gathered (2026-10-03)

- `POST https://vibe.vasandani.dev/mcp` with no credentials →
  `302` to `vasandani.cloudflareaccess.com/cdn-cgi/access/login/...`, with
  `www-authenticate: Cloudflare-Access resource_metadata="…/.well-known/cloudflare-access-protected-resource/mcp"`.
  This is what ChatGPT's "Error creating connector" reports.
- `https://vasandani.cloudflareaccess.com/.well-known/oauth-authorization-server`
  exists but has **no `registration_endpoint`**. ChatGPT requires dynamic
  client registration, so Access cannot be ChatGPT's AS as configured.
- `https://openai.com/chatgpt-connectors.json` (creationTime
  2026-09-22T18:18:05) lists 278 IPv4 prefixes and 0 IPv6.
- Cloudflare provider v5 schema (fetched locally with `tofu providers schema`):
  `cloudflare_zero_trust_list { account_id, name, type, description?,
  items = set({ value, description? }) }`; Access policy include supports
  `ip_list = { id }`.

## Decisions

| Decision | Chosen | Alternatives rejected |
|---|---|---|
| Authorization server | Embedded in VK (opt-in) | **Cloudflare Access managed OAuth**: no DCR advertised, can't be verified or configured from here, and ties tokens to edge-only validation. **Standalone AS service**: a new service to deploy and own, against "manage only VK". |
| Resource path | `/oauth/mcp` | **`/mcp` + OAuth**: bypassing `/mcp` exposes the header-presence trust to forgery from OpenAI ranges, and the path app would drop the service-token policy for other sources. |
| Token check at origin | Caddy `forward_auth` → VK `/api/mcp-oauth/verify` | **Serve MCP natively in VK over Streamable HTTP**: bigger change that duplicates supergateway, which works. **Caddy JWT plugin**: no plugin in nixpkgs Caddy, and opaque tokens need a DB lookup anyway. |
| Token format | Opaque 256-bit random, SHA-256 hash stored | **JWT**: revocation needs a DB lookup anyway. A signing key adds key management. |
| Edge IP set | Zero Trust IP list referenced by `ip_list` | **Inline 278 `ip` includes × 5 apps**: 1,390 rules, near policy-size limits, and noisy plans. |
| Consent UI | Server-rendered HTML in VK | **SPA route**: the SPA would need a new unauthenticated-flow page and API. HTML is smaller and easier to lock down (CSP, no-store, frame-deny). |
| Client auth methods | `none`, `client_secret_post`, `client_secret_basic` | Only `none`: risks rejecting ChatGPT's registration shape. |
| Scopes | single `mcp` | Per-tool scopes (owner chose full access). |
| Lifetimes | code 60 s, consent 10 min, access 1 h, refresh 30 d sliding | Longer access tokens make revocation lag. |

## Standards checklist

- RFC 9728: PRM at `/.well-known/oauth-protected-resource/oauth/mcp` (path-
  suffixed) and at the bare `/.well-known/oauth-protected-resource`. A 401
  carries `WWW-Authenticate: Bearer resource_metadata="<url>"`.
- RFC 8414: AS metadata at `/.well-known/oauth-authorization-server` (issuer
  has no path, so no suffix variant is required). The suffix form is also
  served for clients that append the resource path.
- RFC 7591: `201` with `client_id`, `client_id_issued_at`, `client_secret`
  (+ `client_secret_expires_at: 0`) when confidential, and the echoed
  metadata. Errors use `invalid_redirect_uri` and `invalid_client_metadata`.
- RFC 7636: S256 only, verifier 43–128 chars of `[A-Za-z0-9-._~]`.
- RFC 8707: `resource` validated when present, bound to the token.
- RFC 9207: `iss` in the authorization response.
- RFC 6749 §5.2: token errors are JSON, `invalid_client` → 401 (with
  `WWW-Authenticate: Basic` when Basic was used), others → 400, and
  `Cache-Control: no-store`.
- OAuth 2.1 / RFC 9700: refresh-token rotation with reuse detection, and
  code replay revokes.

## No new dependencies

Everything uses crates already in `crates/server/Cargo.toml` (`sha2`,
`subtle`, `rand`, `base64`, `url`, `serde_json`, `chrono`, `uuid`).
