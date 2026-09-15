# Slack MCP availability in Vibe Kanban sessions

## Problem and scope
Slack MCP tools are unavailable in VK sessions. Diagnose configuration propagation,
launcher startup, authentication and tool discovery using redacted evidence.
Changes are limited to VK and, if needed, its hosting in
`homelab/modules/vibe-kanban-rebuild.nix`; other services require clarification.

## Requirements
- Identify and reproduce the failure before selecting a fix.
- Preserve configured Slack credentials, custom servers and the pinned fork's
  attachment support. Never log credentials or perform Slack message writes.
- Make Slack initialize and expose tools in new and resumed VK sessions.
- Cover the reproduced failure with appropriate regression verification and
  report any remaining live validation limits.
- Follow the requested SpecKit stages, independent review, knowledge recording,
  and pull request/merge workflow.

## Acceptance
A read-only MCP initialization/tool-list probe succeeds under the relevant
session environment, or a precise external blocker is documented. Targeted
checks pass and the reviewed change is merged through a pull request.
