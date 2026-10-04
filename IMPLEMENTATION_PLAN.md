# Implementation plan: ChatGPT OAuth access to the VK MCP

Task `vk/50df-chatgpt-custom-m`. Spec: `SPEC.md`. SpecKit:
`specs/vk/50df-chatgpt-custom-m/`.

## A. vibe-kanban repo

1. **Migration** `crates/db/migrations/20261004000000_mcp_oauth.sql`:
   - `mcp_oauth_clients(client_id PK, client_secret_hash BLOB NULL,
     client_name, redirect_uris JSON, token_endpoint_auth_method,
     created_at)`
   - `mcp_oauth_authorizations(id PK, client_id FK, redirect_uri,
     code_challenge, scope, resource, state NULL, consent_hash BLOB,
     code_hash BLOB UNIQUE NULL, status, grant_id NULL, created_at,
     expires_at)`
   - `mcp_oauth_grants(id PK, client_id FK, scope, resource, created_at,
     revoked_at NULL)`
   - `mcp_oauth_tokens(token_hash BLOB PK, grant_id FK, kind, expires_at,
     revoked_at NULL, created_at, last_used_at NULL)`
   - Indexes on `expires_at`, `grant_id`. FKs `ON DELETE CASCADE`.
2. **Model** `crates/db/src/models/mcp_oauth.rs`: typed rows and small async
   fns (insert client, prune, count, find client, create authorization,
   approve/deny, consume code, create grant + tokens, rotate refresh,
   revoke grant, verify access token, list grants). Use runtime
   `sqlx::query*` and transactions where multi-row. Unit tests use an
   in-memory DB.
3. **Server module** `crates/server/src/mcp_oauth/` (sibling of `mcp_gateway`):
   - `config.rs`: `McpOAuthConfig::from_env()` parses
     `VK_MCP_OAUTH_PUBLIC_URL` → issuer and resource. Pure helpers are tested.
   - `crypto.rs`: random token (32 bytes, base64url), sha256, PKCE S256
     verify, verifier charset check, constant-time eq.
   - `mod.rs`: `public_router()` (`/.well-known/...`, `/oauth/register`,
     `/oauth/authorize`, `/oauth/token`) and `api_router()` (`/mcp-oauth/
     verify`, `/mcp-oauth/grants`, `/mcp-oauth/grants/{id}`). Handlers return
     `404` when config is absent. Consent HTML is rendered inline with
     escaping and security headers.
   - Wire into `routes/mod.rs`: public router merged at the top level before
     the SPA fallback. The API router goes in `relay_signed_routes`, except
     `verify`, which goes in the plain `api_routes` because Caddy calls it on
     loopback without relay signing. (Check how relay signature middleware
     treats unsigned local requests; if it passes them, keep everything in
     one router.)
4. **Tests** (route-level, `tower::ServiceExt::oneshot` against the router
   with an in-memory DB state, or handler-level with pure functions where
   the deployment type is heavy): metadata, DCR validation, authorize
   validation split, consent, PKCE, single-use and replay, refresh rotation
   and reuse, verify, and disabled → 404.
5. Docs: `crates/mcp/AGENTS.md` section "Remote OAuth access (ChatGPT)";
   `docs/` page if an MCP doc exists there.
6. `pnpm run format`, `cargo clippy -p server -p db`,
   `cargo test -p server mcp_oauth`, `cargo test -p db mcp_oauth`.

## B. homelab repo

1. `terragrunt/modules/cloudflare-tunnel/main.tf`: add `bypass_access_ip_lists
   = optional(map(string), {})` (path → list key) plus a module-level
   `ip_lists = map(object({description, cidrs}))`. Create
   `cloudflare_zero_trust_list` per key. Bypass apps whose path has an
   ip_list use `include = [{ ip_list = { id } }]`. Precondition: a path can't
   be in both maps, and a referenced list must exist.
2. Vendor `terragrunt/environments/cloudflare-tunnel-vibe-remote/openai-chatgpt-connectors.json`
   and add `scripts/refresh-openai-connector-ranges.sh`. The env
   `terragrunt.hcl` decodes it into `ip_lists.openai_connectors` and adds
   the five bypass paths mapped to that list.
3. `ci/check-edge-exposure.sh`: treat `bypass_access_ip_lists` keys as
   IP-restricted. Add vibe-specific invariants: the five OAuth paths are
   list-restricted, and `/mcp`, `/oauth/authorize` and `/oauth` are never
   bypass paths.
4. `hosts/think/think2.nix` Caddy `:3343`: a `handle /oauth/mcp*` block before
   `/mcp*` does `forward_auth 127.0.0.1:3334 { uri /api/mcp-oauth/verify }`,
   then `rewrite * /mcp` and `reverse_proxy 127.0.0.1:8787`. Set
   `systemd.services.vibe-kanban-dev.environment.VK_MCP_OAUTH_PUBLIC_URL`.
   (Or use the vibe-kanban-rebuild/vibe-kanban module option if one fits
   better.)
5. Docs: `docs/vibe-kanban-mcp-deployment.md` ChatGPT section,
   `docs/cloudflare-edge-exposure.md` mechanism note.
6. Validation: `bash ci/check-edge-exposure.sh`, `tofu validate` on the module
   copy, `nix-instantiate --parse`, and the existing VK MCP checks
   (`ci/check-vibe-kanban-mcp-connectivity.sh`, `tests/*.nix` if they
   evaluate locally).

## C. Rollout

Merge VK first. The new routes are inert until the env var is set. Then merge
homelab: terragrunt apply creates the list and bypass apps, comin deploys
Caddy and the env var, and the VK server restarts. Verify:
`curl https://vibe.vasandani.dev/.well-known/oauth-protected-resource/oauth/mcp`
(from a non-OpenAI IP: `403`/redirect expected, which is correct), then
loopback checks on think2, then the user creates the ChatGPT connector with
OAuth.
