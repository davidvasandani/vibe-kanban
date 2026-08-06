# Implementation plan

## VAS-356: settings-owned MCP definitions

1. Refresh the VAS-356 specification and constitution with the single-authority
   rule for MCP definitions.
2. Remove remaining `codex mcp add` startup mutations from the homelab Vibe
   Kanban module while retaining required runtime packages.
3. Generalise the focused invariant to reject all native MCP-add commands.
4. Update knowledge, validate Nix evaluation, and complete independent review.
5. Commit, publish, merge, and verify the Vibe Kanban rollout.

## Coordinator affinity work

1. Bring the task branch onto the current Vibe Kanban upstream base that
   contains coordinator placement and the Server Affinity feature.
2. Extend the affinity request and durable operation identity with an explicit
   coordinator intent, reusing the create-time placement rules and rejecting
   contradictory coordinator-plus-worker targets.
3. Teach affinity updates and restart migrations to commit coordinator-local
   placement without invoking worker scheduling or dispatch.
4. Add a localized Coordinator selector option and map it to the explicit
   coordinator target, preserving worker ordering and eligibility behaviour.
5. Add focused backend and frontend coverage for validation, durable replay,
   coordinator placement, option visibility/order, and target serialization.
6. Run formatting, focused tests, and the relevant type/lint
   checks; document any unrelated pre-existing failures.
7. Run SpecKit analysis, implement tasks in dependency order, and keep the task
   checklist synchronized with completed work.
8. Run an independent Codex diff review, address every confirmed significant
   finding, and repeat verification until clean.
9. Update the project knowledge base only if this change yields a reusable
   lesson not already captured by the placement-intent and affinity-migration
   pages; otherwise record that there is no new knowledge to add.
