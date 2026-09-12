# Implementation Plan: Coordinator Workspace Placement

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

The request contract lives in Rust (`crates/db/src/models/requests.rs`) and is exported to TypeScript through `ts-rs` and `crates/server/src/bin/generate_types.rs`. Workspace creation and clustered scheduling are handled in `crates/server/src/routes/workspaces/create.rs`. The shared React create form is `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`, used by both local and remote web entrypoints. The selector uses the shared `@vibe/ui` Select primitive.

## Architecture & Approach

1. Extend `CreateAndStartWorkspaceRequest` with `run_on_coordinator: bool`, using a serde default so requests produced by older clients deserialize as `false`.
2. Introduce a small placement-intent resolver in the workspace creation route. It maps the two wire fields to exactly one of automatic, coordinator, or worker intent and rejects the contradictory coordinator-plus-worker combination.
3. Resolve intent before creating or mutating a workspace. This makes invalid requests fail before repository setup, attachment association, or placement reservation.
4. In cluster mode, keep the existing scheduler/reservation path for automatic and worker intent. For coordinator intent, skip scheduler/reservation so the workspace's initial `local` placement remains authoritative and the existing container start path executes locally.
5. In standalone mode, retain the existing local path. Coordinator intent is accepted; contradictory intent is still rejected consistently at the request boundary.
6. In `CreateChatBoxContainer.tsx`, add a coordinator UI-state sentinel and map UI state to the two contract fields at submit time. Keep worker eligibility and selector visibility behavior unchanged.
7. Extract the frontend mapping into a small pure function for inexpensive exhaustive unit coverage. The create container has no existing rendered-component harness, so avoid duplicating the create-mode application solely for this selector.
8. Regenerate `shared/types.ts`, format, and verify the touched Rust and TypeScript surfaces.

## Data Model

See `./data-model.md`. No database migration is required.

## Contracts

See `./contracts/create-workspace.md`.

## Research Notes

See `./research.md`. No new dependencies are required.

## Constitution Check

- **II — Test the contract:** pure backend intent resolution and frontend serialization are directly testable; the rendered selector is covered where practical.
- **III — Small, reversible steps:** the request addition is additive and the existing scheduler is unchanged.
- **IV — Shared-component boundaries:** behavior remains in `web-core`, while presentation continues to use `@vibe/ui` primitives.
- **VI — Don't rebuild what shipped:** coordinator execution reuses the established local placement and container lifecycle.
- **XVIII — Distributed execution:** automatic, coordinator, and worker intent are explicit and affinity remains persisted for worker placement.
- **XXI — One convention per concept:** the UI and backend each centralize placement-intent resolution rather than duplicating sentinel interpretation across call sites.

No constitution deviations are expected.

## Risks & Dependencies

- A missing serde default would break older clients; a deserialization regression test guards this.
- Validating after workspace creation could leave partial state; intent is resolved at handler entry.
- A coordinator UI sentinel must never cross the API boundary as a UUID; the pure serializer owns this translation.
- `shared/types.ts` is generated and must be updated through `pnpm run generate-types`.
