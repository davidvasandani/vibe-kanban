# Technical Specification: Shared HTTP Slack MCP

Task id: `967a-migrate-slack-mc`

## Summary

Replace the per-agent stdio Slack MCP process with one long-running,
network-reachable Slack MCP server managed alongside the Vibe Kanban cluster.
The coordinator and all workers will configure the bundled `slack` catalog
entry as an HTTP MCP endpoint, so every execution connects to the same service
instead of downloading and launching its own copy.

## Motivation

The bundled Slack MCP currently uses `npx` with the forked release archive and
passes the Slack credential into every launched agent process. That model is
host-local: workers independently start a stdio child and require local secret
configuration. A shared HTTP service gives all cluster workers one stable
endpoint and centralizes the Slack credential and process lifecycle.

## Scope

- Vibe Kanban's bundled Slack MCP catalog entry and its validation/tests/docs.
- `homelab/modules/vibe-kanban-rebuild.nix`, which governs the Vibe Kanban
  coordinator and workers, including the HTTP Slack MCP systemd service,
  secret provisioning, network policy, and endpoint propagation.
- The Vibe Kanban hosts only. No unrelated homelab service is changed.

## Functional requirements

1. Run the pinned `davidvasandani/slack-mcp-server` fork as a persistent HTTP
   MCP service under systemd with automatic restart and startup ordering.
2. Keep Slack credentials outside the Nix store and outside repository files;
   inject them at runtime from the existing Vibe Kanban secret-management
   boundary.
3. Bind the server to a stable private address/port reachable from every Vibe
   Kanban worker, while limiting ingress to the known cluster addresses.
4. Configure the bundled `slack` MCP entry as HTTP, with no stdio command,
   arguments, or per-agent Slack secret environment variables.
5. Preserve the existing Slack MCP tool behavior, including the fork-specific
   attachment support and pinned artifact/version integrity.
6. Ensure coordinator and worker executions resolve the same endpoint without
   requiring users to recreate the bundled catalog entry.
7. Document the ownership, endpoint, authentication boundary, and operator
   verification procedure without exposing credentials.

## Reliability and security

- The service must run as a dedicated, least-privileged identity with a
  restrictive systemd sandbox and no writable application state unless the
  upstream server requires it.
- The endpoint must not be publicly exposed. Host firewall rules must admit
  only the Vibe Kanban cluster paths required by coordinator and workers.
- A failed Slack MCP service must not prevent Vibe Kanban itself from starting;
  agents should receive a normal MCP connection failure until it recovers.
- Logs and generated configuration must not contain Slack token values.
- Existing saved user-defined MCP entries are not rewritten; this migration
  changes the bundled default/catalog definition and deployment default.

## Acceptance criteria

- The Nix module evaluates with the service enabled on the intended Vibe
  Kanban host and its firewall scope is covered by module tests or equivalent
  evaluation assertions.
- The service command launches the pinned fork in its supported HTTP transport
  mode and reads credentials only at runtime.
- Vibe Kanban tests confirm the bundled Slack entry is HTTP and contains no
  executable command, stdio arguments, or Slack secret variable.
- Documentation describes one shared endpoint usable by coordinator and worker
  agents and includes a credential-safe connection check.
- Relevant Rust tests, formatting, Nix formatting/evaluation checks, and the
  independent Codex diff review complete without significant findings.

## Out of scope

- Changes to Slack workspaces, Slack app permissions, or token rotation.
- Public Internet exposure or Cloudflare tunnel creation.
- Changes to any MCP integration other than Slack.
- Changes to services other than Vibe Kanban and its governing deployment
  module.
