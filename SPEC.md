# Technical specifications

## VAS-356: settings own every MCP definition

Vibe Kanban settings are the sole authority for all MCP definitions, including
`vibe_kanban`. Nix may install MCP executables and provide service environment
or network access, but must not invoke `codex mcp add`, remove, or otherwise
mutate native MCP tables during startup. Existing saved settings remain intact,
and remote workers receive selected definitions through the authenticated,
execution-scoped snapshot mechanism. The homelab invariant must reject any
future native MCP seeding in the Vibe Kanban module.

## Coordinator option in workspace “Run on” selector

## Problem

The workspace Server Affinity section exposes automatic placement and worker
hosts in its “Run on” selector, but omits the cluster coordinator. A workspace
can already be explicitly placed on the coordinator when it is created, so the
affinity control must offer the same destination when an operator changes an
existing workspace’s placement.

## Goal

Show a selectable, clearly labelled Coordinator destination in the Server
Affinity “Run on” dropdown and extend the affinity operation with the same
explicit coordinator placement intent used during workspace creation.

## Functional requirements

1. In a cluster-enabled local deployment, the “Run on” selector lists, in a
   stable order:
   - Automatic placement
   - Coordinator
   - every worker returned by the worker-nodes API
2. Selecting Coordinator uses a dedicated coordinator placement intent in the
   migration API contract; it must not masquerade as automatic placement or as
   a worker UUID.
3. Existing worker eligibility behaviour remains unchanged: ineligible workers
   retain their current disabled state, while Coordinator remains available as
   the local execution destination.
4. After migration, the section reports the workspace’s coordinator-local
   placement through its existing local-placement presentation.
5. User-visible text is localized through the existing translation structure.

## Non-goals

- Changing automatic scheduling policy.
- Changing worker registration, health, or eligibility rules.
- Adding the coordinator to the worker-nodes API as a synthetic worker.
- Updating deployment configuration or any service other than Vibe Kanban.

## Acceptance criteria

- Opening Server Affinity shows Coordinator between Automatic placement and the
  worker hosts.
- Choosing Coordinator issues an affinity update with the coordinator target
  and the UI settles on its coordinator-local presentation after the update.
- Unit/component coverage prevents the Coordinator option from disappearing
  and verifies its target mapping.
- Relevant frontend checks and focused tests pass.
