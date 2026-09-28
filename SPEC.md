# SPEC — Migrate the Personal ServiceNow MCP from stdio to HTTPS

Task: `vk/975e-migrate-personal`

## Problem

The bundled **Personal ServiceNow** catalog entry (`personal_servicenow` in
`crates/executors/default_mcp.json`) launches a stdio child process,
`personal-servicenow-mcp`. Every schedulable execution host therefore has to
carry the wrapper, a 1Password-backed credential oneshot and the ServiceNow
OAuth secret. The saved settings entry (Settings → MCP Servers → Edit MCP
server) shows `Transport: stdio (command)`.

A dedicated, always-on Streamable HTTP deployment already runs on think1 at
`https://snow.vasandani.dev/mcp` (homelab `modules/personal-servicenow-mcp.nix`).
It sits behind Cloudflare Access, with a `/mcp` bypass for the reviewed source
CIDRs, plus a mandatory origin bearer. The homelab runbook deferred the VK
switch until public proof exists. It also requires header values to go through
the owning runtime secret boundary, not catalog JSON. Homelab constitution
principle 64 forbids serializing deployment-supplied machine credentials into
settings or native client config.

## Evidence gathered (2026-09-28, from a cluster worker)

- `GET https://snow.vasandani.dev/mcp` with no bearer → `403 Forbidden`, an
  origin rejection. So the `/mcp` Access bypass applies to worker egress.
- `GET https://snow.vasandani.dev/healthz` → `302`: Access SSO still gates
  non-`/mcp` paths.
- `POST initialize` with the origin bearer → `200`, `mcp-session-id` issued,
  `serverInfo.name = personalmcpservicenow`.
- VK already rewrites a settings-owned public MCP URL to the host's loopback
  route when it writes native config (`route_mcp_url_for_runtime`), and maps it
  back on read (`public_mcp_url_for_runtime`).

## Goal

The Personal ServiceNow MCP is consumed over HTTPS (MCP Streamable HTTP)
instead of stdio, and the origin bearer never enters VK settings, native agent
config, catalog JSON, or Git.

1. **Catalog (vibe-kanban):** the bundled `personal_servicenow` tile becomes a
   URL-only HTTP definition, `https://snow.vasandani.dev/mcp`.
2. **Runtime route (homelab):** the existing per-host loopback MCP gateway
   (`services.vibeKanban.protectedMcpRoutes`) is generalised. It can now attach
   an origin `Authorization: Bearer` resolved from a 1Password reference, and
   the Access service-token pair becomes optional. Every VK host (think1–5)
   gets a `personal_servicenow` route on one fleet-wide loopback port.
3. **Credential:** the think1 bearer is mirrored into a Homelab 1Password item
   that the routes reference. The think1 file remains the origin's source, and
   the rotation procedure updates both.
4. **Live settings:** the saved entry is switched to the URL-only HTTP
   definition. No secret is involved.
5. **Docs** in both repos describe HTTPS as primary and stdio as the rollback.

## Non-goals

- Removing the fleet-local stdio wrapper and credential units. They are the
  rollback path; removal is a follow-up after soak.
- Changing the think1 service, the Access policy, the CIDR list or the bearer
  value.
- Any VK Rust code path beyond the catalog and its test.

## Requirements

- **FR-1** `default_mcp.json` `personal_servicenow` MUST be exactly
  `{"type":"http","url":"https://snow.vasandani.dev/mcp"}`. The id and `meta`
  name/description stay unchanged.
- **FR-2** A VK unit test MUST pin that shape, and assert there are no
  `command`/`args`/`env`/`headers` fields.
- **FR-3** `protectedMcpRoutes.<name>` MUST accept an optional
  `bearerTokenRef`. `clientIdRef`/`clientSecretRef` MUST be optional, but
  either both set or both null. Every route MUST carry at least one credential
  (the Access pair or the bearer). Evaluation assertions MUST enforce these
  rules.
- **FR-4** When `bearerTokenRef` is set, the gateway MUST send
  `Authorization: Bearer <value>` upstream, replacing any client-supplied
  Authorization. It MUST validate the value's charset before writing it into
  the runtime Caddyfile, and MUST fail closed with a secret-safe JSON-RPC 401
  when the value is unavailable. Access-only routes MUST keep their current
  rendered behaviour.
- **FR-5** think1–5 MUST declare the `personal_servicenow` route with an
  identical port. `tests/vibe-kanban-cluster.nix` MUST cover a bearer-only
  route and the new assertions.
- **FR-6** Docs: the VK connector subsection and the homelab runbook
  (registration, rotation, rollback) MUST be updated.
- **FR-7** The live settings entry MUST be migrated only after the routes are
  deployed and a routed `initialize` succeeds from a worker.

## Acceptance criteria

- `cargo test -p executors personal_servicenow` passes.
- `default_mcp.json` parses.
- `nix eval --impure --file tests/vibe-kanban-cluster.nix` passes.
- The `think1`..`think5` toplevel drvPaths evaluate.
- `pnpm run format` leaves no diff.
- Endpoint proof: `initialize`, `tools/list` including
  `sn_access_cycle_report`, and invalid-bearer rejection. After deployment,
  the same proof through `http://127.0.0.1:<port>/mcp` with no client header.
- After the live migration, the settings read model shows
  `personal_servicenow` as `http` → `https://snow.vasandani.dev/mcp`, and a new
  session lists the ServiceNow tools.
