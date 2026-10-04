# Prior knowledge: ChatGPT custom-connector access (`vk/50df-chatgpt-custom-m`)

Distilled from the VK wiki (`wiki/`) and the homelab knowledge bases
(`docs/knowledge/`, `docs/knowledge-base/`, `knowledge-base/`). Read-only
recall. Nothing in either base covers OAuth *authorization-server* work or
ChatGPT. Everything below is adjacent knowledge.

## How `/mcp` is served today

- `vibe-kanban-mcp` is stdio-only (`crates/mcp/AGENTS.md`). think2 bridges it
  with **supergateway** (`modules/vibe-kanban-mcp.nix`) on `127.0.0.1:8787`,
  Streamable HTTP, `--stateful`, with a 30 min session timeout. Supergateway
  does **no inbound auth**, so Caddy enforces it
  (`docs/knowledge/mcp-over-http-public-exposure.md`).
- Caddy `:3343` (`hosts/think/think2.nix`) `handle /mcp*` accepts the static
  bearer (`{$VIBE_MCP_BEARER_TOKEN}`) **or the mere presence** of
  `Cf-Access-Jwt-Assertion`. That presence check is only sound while every
  `/mcp` request passes Cloudflare Access. → A ChatGPT bypass must not be on
  `/mcp`.
- The catch-all `handle` proxies to the VK server on `127.0.0.1:3334`. Caddy
  evaluates `handle` blocks in source order with the catch-all last.
- Edge: `vibe.vasandani.dev` is a hostname Access app (email SSO plus Service
  Auth for the `vibe-mcp-client` token and Ohana's token). `/mcp` inherits it.
  Agents use a loopback credential gateway that injects the
  `CF-Access-Client-*` headers
  (`docs/knowledge-base/vibe-kanban-public-mcp-access-routing.md`).

## Edge-exposure rules (homelab)

- `docs/cloudflare-edge-exposure.md` and `ci/check-edge-exposure.sh`: every
  `bypass_access_paths` entry must be IP-restricted
  (`bypass_access_ip_ranges`) or carry a block-level `# edge-open-ok:`. The
  vibe block already has `edge-open-ok` for `/v1…`, so the checker would
  silently pass any new unrestricted vibe path. New paths need an explicit
  check.
- A path-scoped bypass app *replaces* the hostname app for that path
  (most-specific wins). Its only policy is the bypass, so non-matching
  sources (including service-token clients) are **blocked**. This is another
  reason not to bypass `/mcp`.
- Prefer exact CIDRs, and append rather than replace
  (`knowledge-base/cloudflare-access-trusted-source-bypasses.md`).
- Terragrunt apply runs in CI on push to `main` (`runs-on: [self-hosted,
  think2]`). The PR gets a plan
  (`knowledge-base/cloudflare-access-service-token-live-enablement.md`).

## Cloudflare behaviour worth knowing

- Unauthenticated requests get `302` to `*.cloudflareaccess.com` plus
  `www-authenticate: Cloudflare-Access resource_metadata=…`. That is the
  connector-creation failure ChatGPT shows.
- Cloudflare replaces origin **502/504** bodies with branded HTML. Stable
  error contracts must use 4xx/503 (`knowledge-base/edge-safe-error-statuses.md`).
  OAuth errors are 400/401, which is fine.
- The team domain `vasandani.cloudflareaccess.com` publishes AS metadata, but
  without a `registration_endpoint`, so ChatGPT's DCR can't use it.

## VK server conventions relevant here

- Routers: `crates/server/src/routes/mod.rs`. Non-`/api` routes are merged at
  the top level before the SPA `/{*path}` GET fallback (see
  `mcp_gateway::gateway_router`). `/api` has `validate_origin` (rejects
  mismatched `Origin`; requests with no Origin pass).
- `mcp_gateway/mod.rs` is the house pattern for secrets: random bytes, store
  `Sha256` digests, compare with `subtle::ConstantTimeEq`, `URL_SAFE_NO_PAD`
  base64.
- DB models use runtime `sqlx::query_as::<_, T>` (no `.sqlx` offline cache
  entries needed). Tests use `sqlite::memory:` and
  `sqlx::migrate!("../db/migrations")`.
- `mcp_auth.rs` has `html_escape` and simple HTML responses (an OAuth
  *client* flow for upstream MCPs, which is the opposite direction from this
  task).
- Shared-gateway connection IDs must stay stable
  (`wiki/mcp-oauth-connection-identity.md`). Unrelated, but don't touch them.

## Gaps (no prior knowledge)

- MCP authorization spec / ChatGPT connector OAuth requirements (RFC 9728,
  8414, 7591, PKCE).
- Cloudflare Zero Trust lists in Access policies. Verified locally against
  the provider v5 schema: `cloudflare_zero_trust_list{type="IP", items=[{value,
  description}]}` and the Access include `ip_list = { id }`.
