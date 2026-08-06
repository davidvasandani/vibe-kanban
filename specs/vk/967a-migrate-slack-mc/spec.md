# Feature Specification: Shared HTTP Slack MCP

**Feature dir**: `specs/vk/967a-migrate-slack-mc/`
**Status**: Draft

## Summary

Vibe Kanban will provide its bundled Slack connector as one privately hosted
HTTP MCP service shared by the coordinator and every execution worker. This
removes host-local Slack MCP launches and agent-readable Slack credentials while
preserving the fork-specific Slack search, reading, and attachment behavior.

## User Stories

- As a Vibe Kanban user, I want Slack tools to work regardless of which cluster
  worker runs my task so that distributed execution does not change available
  capabilities.
- As an operator, I want one supervised Slack connector so that credentials,
  upgrades, failures, and logs have one controlled boundary.
- As an existing user of the bundled Slack template, I want the migration to
  happen automatically so that I do not have to delete and recreate every
  agent's Slack entry.
- As a security reviewer, I want Slack credentials absent from agent config and
  the connector reachable only from declared Vibe Kanban hosts.

## Functional Requirements

- FR-1: The deployment must expose one Slack MCP endpoint reachable by the Vibe
  Kanban coordinator and all configured execution workers.
- FR-2: The endpoint must use MCP Streamable HTTP rather than stdio or legacy
  SSE.
- FR-3: Every intended supported coding agent must receive an equivalent HTTP
  Slack definition that points to that endpoint.
- FR-4: Slack credentials must be held by the shared service and must not appear
  in bundled catalog data, executor-native config files, command arguments,
  repository content, immutable deployment artifacts, or logs.
- FR-5: Only explicitly declared Vibe Kanban cluster consumers and loopback may
  connect to the private Slack MCP port.
- FR-6: The Slack MCP process must be supervised, restart after failure, and
  remain an optional integration whose failure does not fail Vibe Kanban startup
  or health checks.
- FR-7: The deployment must run the pinned
  `davidvasandani/slack-mcp-server` fork that provides the established Slack
  attachment behavior, and must retain integrity/audit controls for that pin.
- FR-8: Existing saved executor entries that exactly match the previously
  bundled Slack stdio template must be recognized and upgraded to the current
  HTTP template without exposing or carrying forward the old token.
- FR-9: A user-created or otherwise non-exact same-name `slack` entry must not
  be silently rewritten.
- FR-10: Connection tests must complete the MCP initialize and tool-list flow
  against the shared endpoint and report ordinary, actionable failures without
  leaking credentials.
- FR-11: Operators must have documented, credential-safe commands to verify the
  service, private listener, network policy, and MCP tool inventory.
- FR-12: Existing Slack tool names, read-only capability policy, attachment
  bounds, and secret-safe error behavior must remain unchanged by the transport
  migration.
- FR-13: The endpoint used by the catalog must be deployment-configurable so a
  generic Vibe Kanban build does not hard-code this homelab's private address.

## Out of Scope

- Modifying Slack app permissions, workspace policy, or rotating Slack tokens.
- Exposing the Slack MCP endpoint through a public hostname or Cloudflare.
- Changing Slack tool behavior or adding Slack write capabilities.
- Changing any non-Vibe-Kanban service.
- Rewriting arbitrary user-defined MCP entries.
- Making Slack availability a scheduler, worker-health, or Vibe Kanban health
  signal.

## Acceptance Criteria

- [ ] Deterministic repository tests prove the HTTP definition, service command,
      and network policy; after the runtime token is provisioned, an operator
      verifies one coordinator and one per-worker execution can initialize the
      endpoint and list the expected tools.
- [ ] Generated native configurations for every assigned supported agent contain
      the shared HTTP URL and no Slack token, stdio command, or package URL.
- [ ] An exact historical bundled stdio entry upgrades to HTTP, while a modified
      `slack` entry is preserved and surfaced as a conflict/custom definition.
- [ ] The service starts with its runtime credential, restarts after a forced
      failure, and its failure leaves Vibe Kanban services healthy.
- [ ] Listener/firewall evaluation admits loopback and declared cluster consumer
      addresses and rejects undeclared sources in both supported firewall modes.
- [ ] Automated tests prove the fork tag/artifact integrity contract remains
      pinned after the catalog migration.
- [ ] Documentation and logs contain no credential values.
- [ ] Focused Rust and Nix verification and independent Codex review complete
      with no significant findings.

## Open Questions

None. Decisions and rationale are recorded in `clarifications.md`.
