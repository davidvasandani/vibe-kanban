# PRIOR KNOWLEDGE — vk/975e-migrate-personal

Distilled from the two project knowledge bases (homelab `knowledge-base/`,
`docs/knowledge/`, and the runbooks under `docs/`; vibe-kanban
`docs/knowledge-base/` and `wiki/`). This pass was read-only.

## The hosted service already exists (homelab)

- `docs/personal-servicenow-mcp-deployment.md`: think1 runs Supergateway 3.4.3
  (stdio → stateful Streamable HTTP) on `127.0.0.1:8790`. Caddy on
  `172.16.100.101:8191` enforces the origin bearer and proxies only `/mcp`.
  The public URL is `https://snow.vasandani.dev/mcp`. There is no SSE endpoint.
- Cloudflare Access SSO gates the hostname. `/mcp` bypasses SSO only for the
  reviewed Claude/Scott/Camero CIDRs, the same list as lmi/cdp. The origin
  bearer is still mandatory. The bearer is generated at
  `/var/lib/personal-servicenow-mcp/http-token`.
- The runbook defers the VK switch explicitly: migrate the bundled entry only
  after public initialize, tools/list, a bounded read and negative-auth proof.
  Header values go through the owning runtime secret boundary, **not catalog
  JSON**. Test in *new* sessions, because existing sessions keep the inventory
  they started with.
- The fleet-local stdio path (`personalServiceNowMcp` in
  `modules/vibe-kanban-rebuild.nix`, enabled on think1–5) is the rollback path
  and is independent of the hosted service.

## HTTP MCP exposure pattern (homelab `docs/knowledge/mcp-over-http-public-exposure.md`)

- Use Streamable HTTP, not SSE. Supergateway has no inbound auth, so Caddy
  enforces the bearer. The edge rule is IP allowlist **and** origin bearer.
- LAN hairpin: on-LAN clients reach `/mcp` through Cloudflare. That works only
  when their egress IP is allowlisted. Cluster workers egress via Scott's
  residential IP, which is listed. Verified this task: no-bearer `/mcp` gets
  an origin 403, and `/healthz` gets an Access 302.

## VK catalog and settings (vibe-kanban `shared-mcp-configuration.md`, homelab `vk-bundled-mcp-catalog.md`)

- `crates/executors/default_mcp.json` is the catalog: a server map plus
  `meta`. An HTTP entry with a placeholder header already exists (`context7`).
  No test enumerates the catalog. `personal_servicenow` has one pinning test in
  `mcp_config.rs`.
- **Catalog changes do not rewrite native files saved from an older template.**
  Settings are derived from native agent files. A historical-template
  migration (the Slack precedent) is only safe when the replacement needs no
  new secret. A stdio → HTTP migration here would need the bearer, so it must
  be an explicit settings save, not an automatic read-time rewrite.
- `POST /api/mcp-config/shared` takes the *complete* logical server list and
  writes each assigned native profile atomically, keeping a `.bak`. Codex
  accepts Streamable HTTP (`url`/`http_headers`).
- Placeholders are not validated. Document that `YOUR_TOKEN` has to be
  replaced. Never commit real-looking credentials.
- Static-bearer HTTP MCPs (LogMeIn, Firecrawl-browser, Windows MCP) are already
  stored in settings as `type: http` with an `Authorization` header. That is
  the established boundary for an operator-held static bearer.

## Cluster runtime (vibe-kanban `cluster-mcp-runtime-connectivity.md`)

- Persistence, runtime adoption and worker connectivity are separate
  boundaries. A coordinator Test passing does not prove a worker can connect.
  Direct public MCP URLs pass through to workers unchanged.

## Governing principle (homelab constitution 64)

Deployment-supplied machine credentials for a settings-managed public MCP
endpoint are attached only at the final outbound hop. They are never
serialized into settings or native client config. The existing mechanism is
`services.vibeKanban.protectedMcpRoutes`: a per-host Caddy loopback gateway,
`vibe-kanban-mcp-access`, that resolves 1Password refs at start. Today it only
injects Cloudflare Access service-token headers, for `vibe.vasandani.dev` on
port 18901. The lmi/cdp/windows entries that keep a bearer in settings predate
this principle and are not a precedent to copy.
