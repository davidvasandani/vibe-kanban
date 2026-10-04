# Implementation Plan: ChatGPT custom-connector access to the Vibe Kanban MCP

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- **Vibe Kanban** (`davidvasandani/vibe-kanban`): Rust, axum 0.8, SQLite via
  sqlx 0.8 (runtime `query_as`), with existing `sha2`, `subtle`, `rand`,
  `base64` and `url` deps in `crates/server`. No new dependencies.
- **Homelab** (this repo): NixOS (think2 Caddy, VK coordinator service env),
  Terragrunt/OpenTofu with Cloudflare provider `~> 5.0`
  (`terragrunt/modules/cloudflare-tunnel`).
- Topology today: Cloudflare tunnel `vibe-remote` → Caddy `:3343` on think2.
  `/mcp*` goes to supergateway `127.0.0.1:8787` (stdio `vibe-kanban-mcp
  --mode global`), and the catch-all goes to the VK coordinator
  `127.0.0.1:3334`.

## Architecture & Approach

```
ChatGPT (OpenAI egress) ──► Cloudflare edge
   /.well-known/oauth-*, /oauth/register, /oauth/token, /oauth/mcp
        └─ bypass app: include ip_list(openai_connectors) ─► Caddy :3343
   /oauth/authorize (browser) ─► hostname Access SSO ─► Caddy :3343
Caddy :3343
   handle /oauth/mcp* → forward_auth 127.0.0.1:3334 /api/mcp-oauth/verify
                        → rewrite /mcp → reverse_proxy 127.0.0.1:8787 (supergateway)
   handle /mcp*       → unchanged (static bearer | Access assertion)
   handle             → 127.0.0.1:3334 (VK: /.well-known/*, /oauth/*, UI, /api)
```

### Vibe Kanban changes

| File | Change |
|---|---|
| `crates/db/migrations/20261004000000_mcp_oauth.sql` | four tables (see data-model) |
| `crates/db/src/models/mcp_oauth.rs` (+ `models/mod.rs`) | row types and queries, with transactions for code exchange and rotation |
| `crates/server/src/mcp_oauth/{mod,config,crypto,pages}.rs` | config from `VK_MCP_OAUTH_PUBLIC_URL`, handlers, consent HTML |
| `crates/server/src/lib.rs` | `pub mod mcp_oauth;` |
| `crates/server/src/routes/mod.rs` | merge `mcp_oauth::public_router()` at the top level (before the SPA fallback), and `mcp_oauth::api_router()` under `/api` |
| `crates/mcp/AGENTS.md`, `docs/` | remote OAuth section |

Handlers take `State<DeploymentImpl>` and use `deployment.db().pool`. Core
logic lives in functions that take `&SqlitePool` and `&McpOAuthConfig`, so
tests run against `sqlite::memory:` without a full deployment.

`/api/mcp-oauth/verify` must work for an unsigned loopback request from
Caddy. If the relay-signature middleware rejects unsigned non-relay requests,
the verify route is merged into `api_routes` outside `relay_signed_routes`.
(Implementation checks `middleware::require_relay_request_signature`.)

### Homelab changes

| File | Change |
|---|---|
| `terragrunt/modules/cloudflare-tunnel/main.tf` | `variable "ip_lists"` (map of `{description, cidrs}`) → `cloudflare_zero_trust_list` (type `IP`); per-hostname `bypass_access_ip_lists` (path → list key); bypass apps use `include = [{ ip_list = { id } }]`; preconditions: path not in both maps, list key exists, path is in `bypass_access_paths` |
| `terragrunt/environments/cloudflare-tunnel-vibe-remote/terragrunt.hcl` | `ip_lists.openai_chatgpt_connectors` from vendored JSON; vibe adds 5 bypass paths mapped to it |
| `terragrunt/environments/cloudflare-tunnel-vibe-remote/openai-chatgpt-connectors.json` | vendored copy of `https://openai.com/chatgpt-connectors.json` |
| `scripts/refresh-openai-connector-ranges.sh` | refetch, validate (IPv4/IPv6 CIDR, non-empty), rewrite the vendored file |
| `ci/check-edge-exposure.sh` | `bypass_access_ip_lists` keys count as IP-restricted; vibe invariants: the 5 OAuth paths are list-restricted, and `/mcp`, `/oauth`, `/oauth/authorize` are never bypassed |
| `hosts/think/think2.nix` | Caddy `handle /oauth/mcp*` before `handle /mcp*` |
| `modules/vibe-kanban-rebuild.nix` | coordinator env `VK_MCP_OAUTH_PUBLIC_URL = "https://vibe.vasandani.dev"` |
| `docs/vibe-kanban-mcp-deployment.md`, `docs/cloudflare-edge-exposure.md` | ChatGPT section; ip-list mechanism |

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/oauth-endpoints.md` and `./contracts/edge-and-caddy.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- **148 (new)**: separate origin-authorized resource. `/mcp` shortcuts are not
  bypassed, vendor ranges are vendored with a refresh path, consent is
  SSO-gated, tokens are hashed, rotating and replay-revoking, the feature is
  inert until configured, and the checker proves it. ✔
- **8 Exposure matches authentication**: bypass paths limited to a
  documented machine-ingress contract (OpenAI ranges) plus origin OAuth. ✔
- **12 Bound untrusted bytes**: DCR body ≤ 16 KiB, ≤ 10 redirect URIs of
  ≤ 2048 chars, `client_name` ≤ 200, form bodies bounded, client cap. ✔
- **13 Actionable secret-safe errors**: RFC 6749 error codes with guidance
  and no token echo. ✔
- **4/14 Tests**: in-memory DB route tests, deterministic clocks via explicit
  `now` parameters where expiry matters. ✔
- **Edge exposure rule**: checker extended rather than relying on the
  block-level `edge-open-ok`. ✔
- VK constitution **XLIX**, the same properties on the VK side. ✔

No deviations.

## Risks & Dependencies

- **ChatGPT egress outside the published list** → edge 403. Mitigation: the
  vendored list is current (2026-09-22), there is a refresh script, and the
  rollout check is documented.
- **No paging on range drift (principle 7 deviation, accepted).** If OpenAI's
  ranges drift, the edge blocks ChatGPT. The failure is immediately visible
  in ChatGPT, not hidden behind a green check. Follow-up: a scheduled job
  diffing `chatgpt-connectors.json` against the vendored copy, paging via
  ntfy.
- **Cloudflare API token scope** for Zero Trust lists. The module already
  requires `Zero Trust:Edit`, which covers lists. The PR plan in CI
  surfaces any permission gap before apply.
- **Merge order**: VK first (inert), then homelab. If homelab lands first,
  `/oauth/mcp` verify returns the SPA/404 and fails closed. Safe either way.
- **Cloudflare Access may handle `/.well-known/` specially.** It intercepts
  `/.well-known/cloudflare-access-protected-resource`. Our paths differ, and
  the post-deploy curl from an OpenAI-range-equivalent probe isn't possible.
  We verify with ChatGPT itself, and origin behaviour on think2 loopback.
- **supergateway session behaviour with ChatGPT**: same transport other
  clients already use. Rewrite keeps the `Mcp-Session-Id` header untouched.
