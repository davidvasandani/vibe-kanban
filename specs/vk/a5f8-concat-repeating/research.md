# Research: Background Workspace Creation

## Current cancellation boundary

`create_and_start_workspace` in `crates/server/src/routes/workspaces/create.rs` awaits repository association, remote attachment/context calls, placement, worktree creation, and `start_workspace` before responding. The frontend mutation remains mounted and shows “Creating…” throughout. Dropping the HTTP request can drop this handler future at any await, so the workspace row alone is not evidence that creation completed.

## Decision: workspace-owned lifecycle state

Creation is a one-time lifecycle of a workspace, and existing workspace list/detail reads already drive navigation. Storing its status on `workspaces` is the smallest authoritative model and automatically gives clients an observable identity before sessions exist.

A separate general job table was rejected because this feature does not expose scheduling, retry, history, or multiple job kinds. It would add joins and APIs while still requiring a workspace-level summary for the UI.

## Decision: runtime-owned task plus startup reconciliation

Tokio ownership severs browser cancellation from the work. Persisting `queued` before spawn and claiming it atomically prevents two live consumers. Tokio tasks do not survive process shutdown, so startup reconciliation turns unproven unfinished operations into visible failures rather than leaving them pending or replaying non-idempotent Git work.

Full phase checkpoint/replay was rejected for this increment. Repository association, attachment import, placement, filesystem materialization, and process startup do not currently share a phase-idempotency contract. Replaying them after an arbitrary crash risks duplicate initial execution or destructive worktree behavior.

## Decision: return workspace, not speculative execution

An execution does not exist at acceptance time. Returning an optional or placeholder execution weakens the contract. The response returns the accepted workspace only; clients observe the real execution through existing session/execution reads after creation reaches ready.

## Error policy

Persist a bounded generic-but-actionable message that names workspace creation as the failed operation. Log the full error with workspace ID and phase. This keeps server paths, remote bodies, and potential configuration details out of user-visible durable state.

No new dependency is required.
