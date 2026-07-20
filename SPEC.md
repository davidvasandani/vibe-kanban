# Technical Specification: Organization Environment Variables in Workspaces

Task: `vk/6d24-org-env-vars-are`

## Problem

Organization environment variables are stored and injected into managed coding
agent, setup, and development-server processes, but the interactive terminal in
a workspace is spawned through a separate PTY service. As a result, opening the
workspace terminal does not expose the organization variables shown in
Organization Settings.

## Required Behavior

- Resolve organization variables using the workspace's task, local project, and
  remote project association.
- Make the resolved variables available to newly opened workspace terminals.
- Keep the existing best-effort behavior: local-only workspaces and remote
  lookup failures still open a terminal without organization variables.
- Bound remote resolution with the existing timeout.
- Reject application-reserved keys (`VK_*`, `PATH`, `HOME`, `LD_PRELOAD`,
  `LD_LIBRARY_PATH`, and `OPENCODE_SERVER_PASSWORD`).
- Apply terminal-owned values after organization values so `TERM`, `COLORTERM`,
  prompt setup, and `VIBE_KANBAN_TERMINAL` retain their runtime meanings.
- Never log environment values.

## Design

Expose organization environment resolution through the existing
`ContainerService` interface and keep its implementation in
`LocalContainerService`, which already owns workspace-to-project mapping,
authenticated remote access, timeout behavior, filtering, and warnings.

The terminal WebSocket route resolves the map for the authenticated workspace
and passes it by value into `PtyService`. PTY command creation applies the map to
the child command before applying terminal-owned values. Managed CLI login PTYs
continue to pass an empty explicit map and retain their cleared, allowlisted host
environment.

No schema, remote API, generated type, or frontend change is required.

## Verification

- A PTY child test confirms an organization-style value is visible to the child.
- The same test confirms an attempted `TERM` override loses to the PTY contract.
- A resolver test confirms reserved keys are rejected and ordinary credential
  names are accepted.
- `cargo check -p server` validates the terminal route and service integration.
- `cargo fmt --all` and `git diff --check` validate formatting.
