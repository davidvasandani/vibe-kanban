# Technical Spec: Expose Vibe Kanban CLI Tools in Workspace Sessions

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
