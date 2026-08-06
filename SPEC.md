# Technical Spec: Expose Vibe Kanban CLI Tools in Workspace Sessions

## VAS-356 Follow-up: Remote MCP Configuration Synchronization

### Problem

The coordinator owns the MCP definitions edited in Vibe Kanban settings, but a
cluster worker launches the selected coding agent from its own native config
file. Deployment bootstrap can install an MCP command on a worker, but it cannot
reproduce settings-owned headers or environment secrets. Consequently the
worker's Firecrawl stdio client reaches its backend and fails scope bootstrap
with HTTP 401 even though the coordinator-side MCP definition is authenticated.

### Required behavior

1. A remotely dispatched execution receives the MCP server definitions for its
   selected executor profile from the coordinator.
2. The worker materializes those definitions in the selected executor's native
   MCP config before starting the coding agent, using the existing adapters and
   atomic config writer.
3. Headers and environment values are transmitted only inside the existing
   authenticated, signed coordinator-to-worker dispatch channel and are never
   logged.
4. The worker rejects an MCP configuration that does not correspond to the
   dispatched executor profile or exceeds a conservative size bound.
5. Local execution and workers receiving an older dispatch without an MCP
   snapshot retain their current behavior.
6. Applying a snapshot updates only the MCP server section and preserves other
   native agent settings.

### Acceptance criteria

- A Codex job dispatched to think3/think4 sees the same Firecrawl definition as
  coordinator Codex and bootstraps without HTTP 401.
- Focused tests prove profile selection, protocol round-trip, bounded payloads,
  native-config preservation, and secret-safe diagnostics.
- No Firecrawl bearer is copied into Git, the Nix store, or command arguments.
- No non-Vibe-Kanban service is changed.

## Summary

Vibe Kanban can install pinned vendor CLI tools into its app-managed CLI tools
directory, but every newly spawned workspace session must also be able to find
those tools through `PATH`. Extend workspace-session environment construction so
the app-managed `cli-tools/bin` directory is present wherever a Vibe Kanban
workspace session executes, including clustered worker execution.

## Problem

The CLI Tools settings page can report that a tool is installed by Vibe Kanban,
while a workspace session cannot invoke that tool by its command name. Local
agent spawning already appends the app-managed bin directory, but the contract
is not consistently carried into all workspace-session execution paths. This is
especially relevant when the coordinator creates a session that runs on a
cluster worker.

## Required Behavior

1. New workspace sessions receive a `PATH` containing Vibe Kanban's app-managed
   CLI tools bin directory when that directory is available to the execution
   host.
2. Existing machine-provided path entries retain precedence over app-managed
   tools, matching the CLI Tools UI promise that a machine copy wins.
3. Existing `PATH` entries are preserved and duplicate entries are avoided.
4. The behavior applies to the normal local execution path and the clustered
   worker execution path used by Vibe Kanban workspace sessions.
5. Missing app-managed tool directories do not prevent a workspace session from
   starting.
6. The change does not expose the broader app data directory or credentials;
   only the designated CLI tools `bin` directory is added.

## Scope

In scope:

- Vibe Kanban workspace-session environment assembly.
- Cluster protocol/worker handling if required to preserve the environment
  contract across coordinator-to-worker execution.
- Focused automated coverage for path ordering, preservation, and absence.
- Vibe Kanban deployment configuration in
  `../homelab/modules/vibe-kanban-rebuild.nix` only if runtime packaging or
  shared-path availability requires it.

Out of scope:

- Changes to any non-Vibe-Kanban service.
- Adding new CLI tools or changing their pinned versions, installers, login
  flows, credential storage, or settings UI.
- Mutating a running session's environment after it has started; the guarantee
  applies when a session process is spawned.

## Design Constraints

- Reuse the canonical CLI tools directory helper and platform-aware PATH merge
  behavior rather than duplicating path construction.
- Host paths must precede the app-managed bin path.
- Coordinator paths must not be blindly sent to a worker when they are not valid
  on that worker; the execution host must derive or receive a valid tool path.
- Preserve the existing reserved-environment-variable protections.
- Keep deployment changes confined to the Vibe Kanban service's governing Nix
  module.

## Acceptance Criteria

- A workspace session can resolve an app-installed CLI by command name after the
  tool is installed and the session is newly spawned.
- If the same command exists on the machine and in the app-managed directory,
  the machine command resolves first.
- A pre-existing custom `PATH` remains intact and gains at most one canonical
  app-managed bin entry.
- A missing CLI tools bin directory leaves `PATH` unchanged and session startup
  succeeds.
- Focused tests cover the applicable local and clustered workspace-session
  paths.
- Repository formatting and relevant Rust/Nix checks pass.

## Risks

- A coordinator-local asset path may not exist on a worker; deriving the path at
  the wrong layer can produce a misleading but unusable `PATH` entry.
- Shell startup files can rewrite `PATH`; tests must validate the environment
  handed to the spawned process rather than assume interactive shell behavior.
- Changing shared protocol types may require coordinated server/worker updates
  and generated contract verification.
