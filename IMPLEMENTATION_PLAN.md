# Implementation Plan: Durable Background Workspace Creation

1. Trace every create-and-start caller, workspace/session/execution query, event stream, and startup reconciliation path; identify the smallest persisted state model that can represent accepted, running, succeeded, and failed creation.
2. Add a database migration and model for workspace-creation job state and the serialized inputs needed after HTTP acceptance or coordinator restart. Enforce one creation job per workspace and explicit terminal error metadata.
3. Refactor the current `create_and_start_workspace` workflow into validation/acceptance and an idempotent background runner. Preserve placement resolution, repository association, remote attachment/context import, materialization, and execution startup ordering.
4. Add a coordinator-owned runner that claims queued jobs, executes them outside request cancellation, records phase/outcome, and reconciles unfinished jobs on startup. Prevent duplicate initial execution on retries.
5. Change the create-and-start response contract to return the accepted workspace and creation status without waiting for an execution process. Regenerate shared TypeScript types and update all Rust request/response constructors.
6. Expose creation status through the workspace read/event surface used by the frontend, including a safe user-facing failure message.
7. Update create-workspace mutations to navigate as soon as acceptance returns. Update workspace views to render persisted pending/failure states and converge on normal session/execution UI after success.
8. Add backend tests for early acknowledgement, request cancellation independence, state transitions, restart recovery, failure recording, and idempotency. Add frontend coverage for navigation and pending/failure rendering.
9. Run generated-type checks, focused Rust and frontend tests, formatting, lint/type checks, then the independent Codex diff-review loop.
10. Update the Vibe Kanban knowledge base and index with the reusable background-job lifecycle pattern, commit it, and merge the task branch into its base branch.
