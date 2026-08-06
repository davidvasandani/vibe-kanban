# Implementation Plan: Workspace Server Affinity and Migration

**Task:** `vk/9a64-vk-workspace-aff`

This pre-SpecKit plan is grounded in `SPEC.md`, `PRIOR_KNOWLEDGE.md`, and the current placement/execution/UI seams. SpecKit stages may refine paths and contracts before implementation.

## 1. Resolve product and lifecycle semantics

1. Trace workspace summary, placement reservation, stop, session follow-up, and right-sidebar persistence paths end to end.
2. Resolve the open questions in `SPEC.md`: automatic-placement timing, coordinator selectability, authoritative restart session/config, and dev-server behavior.
3. Record decisions in the SpecKit feature specification and supporting research rather than leaving behavior implicit in frontend code.

## 2. Define shared backend contracts

1. Add Rust request/response/outcome types for an affinity mutation and register their TS exports in `crates/server/src/bin/generate_types.rs`.
2. Include compact resolved affinity/placement data in the bulk workspace summary response so the left drawer does not issue one placement query per row.
3. Add a workspace-scoped affinity route under `/api/workspaces/{id}`.
4. Regenerate `shared/types.ts` through `pnpm run generate-types`.

## 3. Implement placement mutation primitives

1. Add DB/service methods that update requested affinity and placement consistently in a transaction.
2. Validate explicit targets against the authoritative worker inventory and scheduler eligibility rules.
3. Define automatic-placement behavior explicitly and cover local/coordinator placement without inventing a worker UUID.
4. Add a per-workspace serialization/idempotency guard so concurrent changes cannot race or duplicate a restart.
5. Unit-test no-op, unknown target, ineligible target, automatic choice, explicit worker choice, and concurrent request behavior.

## 4. Implement managed running-workspace migration

1. Detect all relevant running processes server-side and identify the active coding-agent session/executor configuration.
2. Reject a non-confirmed affinity change with a typed conflict when a task is still running.
3. On confirmed migration, stop active execution through the existing lifecycle service and wait for terminal state before changing placement.
4. Apply the new requested affinity and release/re-reserve placement according to the clarified semantics.
5. Start exactly one coding-agent follow-up using a version-controlled Vibe Kanban continuation prompt and the authoritative session configuration.
6. Return a typed complete or partial outcome; a restart failure must report that the workspace is stopped on the new affinity.
7. Add integration tests covering stopped reassignment, required confirmation, successful stop/reassign/restart, stop failure, affinity failure, restart failure, and duplicate-confirmation idempotency.

## 5. Expose affinity through frontend data hooks `[P after contracts]`

1. Extend the workspace-summary mapper/type with compact affinity data and ensure streamed/refetched summaries retain it.
2. Add a shared affinity-label resolver for local, automatic, assigned, offline, and unassigned states.
3. Add a query/mutation hook for detailed placement, worker inventory, and affinity updates with deliberate cache updates/invalidation.
4. Add API client methods for the affinity endpoint and typed conflict/partial-outcome handling.

## 6. Add the left-drawer affinity label `[P with right drawer after hooks]`

1. Extend the `@vibe/ui` workspace row data shape/presentation with a compact server label.
2. Feed it from bulk workspace summaries in `WorkspacesSidebarContainer` for active and archived groupings.
3. Preserve current name/status/diff layout at narrow drawer widths.
4. Add focused component/mapping tests for worker, local, automatic/unassigned, and missing-worker labels.

## 7. Add the right-drawer Server Affinity accordion `[P with left drawer after hooks]`

1. Add a persisted expansion key and section in `RightSidebar` near Server Metrics.
2. Implement a container that shows current affinity and a creation-style **Run on** selector.
3. Reuse the creation selector's eligibility semantics or extract a shared worker-option helper so the two surfaces cannot drift.
4. For stopped workspaces, submit directly and refresh summary/placement/execution caches.
5. For running workspaces, stage the selected target and show a confirmation dialog describing stop, migrate, and restart.
6. Cancel without mutation; confirm once; disable duplicate actions; render precise complete/partial errors.
7. Add localized strings to every supported locale and component tests for expansion, options, stopped mutation, running cancel/confirm, and failures.

## 8. Integrate and verify

1. Install dependencies with `pnpm install --frozen-lockfile` if this worktree has not been prepared.
2. Run focused Rust and frontend tests while implementing.
3. Run generated-type verification, formatting, frontend/backend checks, lint, and the relevant workspace tests.
4. Exercise the UI at drawer width with stopped and running workspaces on coordinator and worker placements; verify terminal/editor/preview routing follows the new placement.
5. Confirm no changes are required outside the Vibe Kanban service; touch `homelab/modules/vibe-kanban-rebuild.nix` only if this feature introduces an actual deployment requirement.

## 9. Independent review and knowledge capture

1. Run the repository's independent Codex review flow against the task diff.
2. Fix confirmed significant findings and repeat review plus focused/full verification until none remain.
3. Add or update a durable workspace-affinity/migration knowledge-base page tagged `9a64-vk-workspace-aff`, refresh the knowledge index, and commit the knowledge-base change before handoff.
