# Technical Spec: Provide Bubblewrap to Cluster Worker Agents

Task id: `8475-bubblewrap-missi`

## Summary

Add Nixpkgs `bubblewrap` to the systemd `PATH` of the Vibe Kanban cluster
worker service. Agent processes launched by `vibe-kanban-worker` inherit that
unit environment; making `bwrap` available there satisfies Codex's Linux
sandbox prerequisite on worker nodes.

## Problem

The regular and development Vibe Kanban services already include `bubblewrap`
in their Nix `path`, but `vibe-kanban-worker.service` does not. Codex sessions
dispatched to worker nodes therefore report that bubblewrap cannot be found on
`PATH` and fall back to a bundled copy. This produces a visible runtime error
and makes the worker execution environment inconsistent with the coordinator.

## Scope

- Update only `homelab/modules/vibe-kanban-rebuild.nix`, the deployment module
  governing clustered Vibe Kanban workers.
- Add `bubblewrap` to `systemd.services.vibe-kanban-worker.path`.
- Add or extend an evaluation-level regression check proving the rendered
  worker service path contains the Nixpkgs bubblewrap package.
- Update Vibe Kanban deployment knowledge only if the result is reusable.

## Out of Scope

- Changes to any service other than Vibe Kanban.
- Changes to the Vibe Kanban application or executor source.
- Installing bubblewrap globally or changing the host-wide package set.
- Changing Codex sandbox behavior, flags, permissions, or fallback logic.
- Deploying or switching a live NixOS host.

## Requirements

1. On a host configured with `services.vibe-kanban-rebuild.clusterRole =
   "worker"`, `vibe-kanban-worker.service` must have `bwrap` on `PATH`.
2. Existing worker tools and service hardening must remain unchanged.
3. Standalone and coordinator behavior must remain unchanged.
4. The package must come from the pinned Nixpkgs input, matching the existing
   Vibe Kanban service declarations.
5. A repository check must fail if bubblewrap is later removed from the worker
   service path.

## Design

Insert `bubblewrap` into the existing `with pkgs; [ ... ]` list assigned to
`systemd.services.vibe-kanban-worker.path`. NixOS converts that package list
into the unit's generated executable search path, which is inherited by the
worker daemon and the agent subprocesses it launches. No new option is needed:
all cluster worker nodes require the same sandbox prerequisite.

Validation should evaluate the module's worker configuration and inspect the
rendered service path or generated unit environment. A lightweight static
assertion is acceptable only if the repository's existing module-test style
does not support isolated evaluation of this service.

## Acceptance Criteria

- The worker unit path contains the `bubblewrap` package alongside its current
  tools.
- Relevant Nix formatting/evaluation or repository checks pass.
- The diff contains no unrelated service changes.
- Independent review reports no significant findings.

## Risks and Mitigations

- **Wrong execution boundary:** Installing on the coordinator does not help a
  remote worker. The change targets the worker unit that launches Codex.
- **Unnecessary global exposure:** A host-wide package would broaden scope. The
  package is confined to the service path.
- **Regression by list drift:** A focused check documents and enforces the
  worker runtime dependency.
