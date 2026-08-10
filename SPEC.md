# Technical Spec: Durable Background Workspace Creation

## Problem

`POST /api/workspaces/start` currently performs repository setup, placement, workspace materialization, and initial execution startup inside the request that the create-workspace UI awaits. These operations can take more than ten seconds. Navigating away aborts the browser request and can cancel the server-side future before the workspace has been fully created, leaving the intended workspace unavailable.

## Objective

Make create-and-start durable with respect to the initiating HTTP connection. Once the server accepts a valid request and creates the workspace record, the remaining creation and initial-start workflow must continue as a server-owned background job even if the client disconnects or navigates elsewhere.

## Functional Requirements

1. The create-and-start endpoint validates all request fields that can be validated synchronously and creates a durable workspace identity before acknowledging the request.
2. Slow work—repository association, attachments and remote context import, worker placement, filesystem/worktree materialization, and initial execution startup—runs independently of the request future.
3. The endpoint returns promptly with the created workspace identity and a representation of its pending creation state; the frontend navigates to that workspace without waiting for materialization.
4. Workspace APIs expose enough state for the UI to distinguish pending creation, successful readiness/execution, and failed creation.
5. The workspace page displays a creation-in-progress state while the job runs and updates when the background work completes, without relying on the create form remaining mounted.
6. Failures are persisted and presented to the user instead of leaving an indefinite “Creating…” state.
7. Existing placement choices, repository targets, linked-issue behavior, attachment import, executor configuration, and initial prompt semantics remain intact.
8. Duplicate client retries must not start multiple initial executions for the same accepted workspace.

## Design Direction

Use the service's existing durable background-task/job abstractions if available. Split create-and-start into a short acceptance phase and a background execution phase. Persist job inputs or all state required to resume/observe the operation before returning. Drive frontend progress from persisted server state through the existing query/event refresh mechanisms rather than component-local mutation state.

The exact API and data-model changes will be selected after inspecting existing job, execution, and workspace-state conventions. Backward compatibility should be preserved where practical, but correctness on disconnect takes precedence over preserving the endpoint's synchronous completion semantics.

## Error Handling and Recovery

- Validate prompt, repositories, and contradictory placement options before acceptance.
- Persist a terminal failure state with a safe, actionable error message if background creation fails.
- Log the detailed server error with workspace/job identifiers.
- Ensure partially created resources follow the existing cleanup/reconciliation policy.
- On process restart, accepted but unfinished work must either resume through a durable queue or be deterministically reconciled to a visible failed/retryable state; an in-memory detached task alone is insufficient.

## Testing

- Endpoint tests prove that acceptance returns before a deliberately blocked creation operation finishes.
- A disconnect/cancellation test proves accepted creation continues independently of the request.
- Background-job tests cover success, failure persistence, and idempotent execution.
- Frontend tests cover pending, completion, and failure rendering/navigation behavior.
- Existing create-and-start, placement, linked issue, and attachment behavior remains covered.

## Out of Scope

- Changes to services other than Vibe Kanban.
- Changes to Vibe Kanban deployment or hosting unless required to activate an in-repo background worker safely.
- General redesign of workspace creation UI or unrelated job infrastructure.

## Acceptance Criteria

1. After the server accepts create-and-start, closing or navigating away from the create view does not prevent workspace creation.
2. The create request is no longer held open for the full worktree/materialization duration.
3. The resulting workspace and initial execution have the same configuration and content as in the current synchronous flow.
4. Pending and failed states are observable and do not remain ambiguous indefinitely.
5. Automated tests demonstrate request-lifetime independence and cover relevant state transitions.
6. Repository checks, formatting, and independent Codex review pass with no significant findings.
