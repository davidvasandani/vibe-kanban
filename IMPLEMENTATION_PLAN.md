# IMPLEMENTATION PLAN — Migrate the Personal ServiceNow MCP from stdio to HTTPS

Task: `vk/975e-migrate-personal` · Spec: `SPEC.md` · SpecKit artifacts:
`homelab/specs/vk/975e-migrate-personal/`

## Step 0 — Pre-cutover proof

Done so far: no-bearer 403, `/healthz` 302, bearer `initialize` 200.

Still to capture: `tools/list` (includes `sn_access_cycle_report`) and an
invalid-bearer rejection. Read the bearer over ssh into a shell variable and
never echo it.

## Step 1 — Credential mirror

Create the Homelab 1Password item `Personal ServiceNow MCP HTTP Bearer`, with
the think1 bearer in its `credential` field. Pipe the value; never print it. If
the service account cannot write, hand this to the operator and ship the
routes anyway: they fail closed with the 401 diagnostic.

## Step 2 — Generalise `protectedMcpRoutes` (homelab `modules/vibe-kanban-rebuild.nix`)

- Options: make `clientIdRef`/`clientSecretRef` `nullOr str` (default `null`)
  and add `bearerTokenRef` (`nullOr str`, default `null`).
- Assertions: the Access refs are both set or both null; each route has the
  Access pair or a bearer; every set ref starts with `op://`.
- Gateway script: resolve only the configured refs and validate each charset
  (bearer: `^[A-Za-z0-9._~+/=-]+$`). Emit the `header_up CF-Access-*` lines only
  for Access routes, and `header_up Authorization "Bearer …"` only for bearer
  routes. The Access-auth error message stays for Access routes; a
  bearer-specific message covers bearer routes. `unset` every value.

## Step 3 — Hosts and tests (homelab)

- think1–5: add `personal_servicenow = { publicUrl = "https://snow.vasandani.dev/mcp"; port = 18903; bearerTokenRef = "op://Homelab/Personal ServiceNow MCP HTTP Bearer/credential"; };`
- `tests/vibe-kanban-cluster.nix`: add a bearer-only route to the fixture.
  Assert the runtime routes JSON, the rendered gateway script (bearer header
  present, no CF headers for that route), and failures for half an Access pair
  and for a route with no credential.

## Step 4 — Catalog and test (vibe-kanban)

- `default_mcp.json`: `"personal_servicenow": {"type":"http","url":"https://snow.vasandani.dev/mcp"}`.
- `mcp_config.rs`: rename the test to
  `personal_servicenow_catalog_uses_the_hosted_https_endpoint`. Pin the exact
  object and assert the stdio fields and `headers` are absent.

## Step 5 — Docs

- VK `docs/integrations/mcp-server-configuration.mdx`: add a
  `### Personal ServiceNow connector` subsection. Cover the URL-only entry,
  that the deployment attaches the bearer, the manual-header option for hosts
  without a route (never commit it), new sessions, and stdio rollback.
- Homelab `docs/personal-servicenow-mcp-deployment.md`: update the intro,
  registration, rotation (1Password mirror + restart `vibe-kanban-mcp-access`
  fleet-wide) and rollback sections.

## Step 6 — Verify

```bash
cargo test -p executors personal_servicenow && cargo clippy -p executors --all-targets
nix eval --impure --file tests/vibe-kanban-cluster.nix
for h in think1 think2 think3 think4 think5; do nix eval --raw .#nixosConfigurations.$h.config.system.build.toplevel.drvPath; done
pnpm run format
```

## Step 7 — Review, knowledge, PRs, cutover

1. Run the Codex review and update the knowledge base.
2. Open and merge the homelab PR first, then wait for the GitOps rollout.
   Verify `vibe-kanban-mcp-access` is active and a routed `initialize`
   succeeds from a worker.
3. Merge the VK PR.
4. Migrate the live settings entry through `POST /api/mcp-config/shared`,
   changing only this server and keeping its assignments.

## Rollback

Restore the saved entry to `command: personal-servicenow-mcp` from Settings;
the stdio wrapper is still deployed. Revert the catalog commit. The routes are
harmless when unused.
