# Implementation Plan: Background Workspace Creation

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Backend: Rust 2024, Axum, Tokio, SQLx/SQLite in `crates/server`, `crates/services`, `crates/local-deployment`, and `crates/db`.
- Frontend: React/TypeScript, TanStack Query, generated `ts-rs` contracts in `packages/web-core` and `shared/types.ts`.
- Existing flow: `crates/server/src/routes/workspaces/create.rs` creates the workspace record and then performs every slow step inline before returning an `ExecutionProcess`.
- Existing observability: `Workspace` is returned by workspace APIs; database hooks feed existing event/query refresh paths. Extending that model avoids a separate job-status API.
- Constraints: Vibe Kanban only, no new dependency, generated types are regenerated rather than edited, and worktree administration remains coordinator-owned.

## Architecture & Approach

### 1. Persist creation lifecycle on the workspace

Add nullable/defaulted `creation_status` and `creation_error` columns to `workspaces` through a migration in `crates/db/migrations`. Extend `crates/db/src/models/workspace.rs` with a closed `WorkspaceCreationStatus` enum and helpers that atomically claim `queued -> running` and write `ready` or `failed`. Existing rows migrate as `ready`.

The workspace ID is the operation identity. A compare-and-set claim is the single-consumer boundary; slow work occurs after the claim without holding a lock or transaction.

### 2. Split acceptance from execution

In `crates/server/src/routes/workspaces/create.rs`, retain pre-mutation validation for prompt, repositories, and `PlacementIntent`. Create the workspace row in `queued`, clone the deployment and accepted input into a Tokio-owned task, and return the new workspace immediately.

Move repository association, attachment handling, linked-project context, placement reservation, `start_workspace`, and success analytics into an internal runner. The runner first claims the workspace, records `ready` only after `start_workspace` returns, and catches/logs any error before recording a bounded safe failure message. The returned task handle is intentionally owned by the Tokio runtime, not the HTTP request.

At startup, reconcile `queued`/`running` workspaces conservatively: if an initial execution exists, mark ready; otherwise mark failed/interrupted with an actionable message. This closes crash windows without replaying partially completed Git operations.

### 3. Change the response and read contract

Change `CreateAndStartWorkspaceResponse` in `crates/db/src/models/requests.rs` to contain only the accepted `Workspace`; creation fields on `Workspace` make its status observable through existing list/detail APIs. Regenerate `shared/types.ts` via `pnpm run generate-types` and update Rust/MCP callers that currently expect `execution_process`.

### 4. Render authoritative creation state

`packages/web-core/src/shared/hooks/useCreateWorkspace.ts` already consumes only `workspace`, so it naturally navigates after the shortened request. Update direct callers in `WorkspacesLayout.tsx`, `VSCodeWorkspacePage.tsx`, and the MCP task-attempt path for the new response semantics.

At the workspace layout/content boundary, render a small pending or failed state before components that assume repositories and sessions exist. Status comes from the normal workspace query/cache and transitions to the existing UI when `ready`. Include a focused component test for queued/running/failed/ready behavior.

## Data Model

See [`data-model.md`](data-model.md).

## Contracts

See [`contracts/background-workspace-creation.md`](contracts/background-workspace-creation.md).

## Research Notes

See [`research.md`](research.md). No new dependency is introduced.

## Constitution Check

- Principle XII: a workspace-scoped compare-and-set is the authoritative asynchronous claim; no lock spans slow work.
- Principle XVIII: placement and execution remain coordinator-authoritative, affinity-bound, and idempotent by existing workspace/execution identities.
- Principle XXVIII: acceptance persists identity/status before returning; runtime-owned work outlives request cancellation; startup reconciliation produces a truthful terminal result.
- Principles III and VI: status lives on the existing workspace read model and uses the existing create/start implementation rather than introducing a general queue framework.
- Constraint on generated files: Rust DTOs remain authoritative and `shared/types.ts` is regenerated.

No constitution deviations remain.

## Risks & Dependencies

- Some workspace screens assume repositories or sessions exist; the creation-state guard must sit above all such assumptions.
- Axum/Tokio runtime shutdown cancels detached tasks; startup reconciliation must cover both queued-before-spawn and running-during-shutdown states.
- Errors may contain repository paths or external details. Persist a bounded user-safe summary and keep full detail in structured logs.
- Existing MCP/API callers may require an execution ID immediately. They must either poll the accepted workspace until ready or use an existing execution lookup; no caller may reconstruct a second start.
