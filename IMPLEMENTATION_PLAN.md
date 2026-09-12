# Implementation Plan: Recover Wedged MCP Sessions

## 1. Establish the current contracts

1. Trace the existing MCP refresh coordinator, executor capability handoff,
   restart reservation, queued continuation, cluster worker routing, runtime
   server snapshots, `/mcp` UI, and Debug issue creation end to end.
2. Confirm the exact current behavior for every executor, especially
   `CLAUDE_CODE`, with focused tests at the container/executor boundary.
3. Identify the smallest durable restart state needed for local and clustered
   execution without introducing a second process owner.

## 2. Define shared recovery and discovery models

1. Add generated Rust/TypeScript DTOs for restart requests/results and runtime
   MCP server outcomes.
2. Represent restart disposition (`accepted`, `in_progress`, `completed`,
   `failed`), restart scope, timestamps, and safe errors.
3. Extend per-server snapshots with explicit lifecycle state, registered tool
   count, bounded error history/timestamps, and discovery-attempt count while
   preserving rolling-worker deserialization compatibility.
4. Centralize allow-listed error mapping and secret-redaction tests.

## 3. Implement session restart orchestration

1. Promote the existing “restart agent for MCP changes” queue/reservation path
   into a container-service operation callable outside the browser component.
2. Validate workspace/session ownership and executor configuration server-side.
3. For idle sessions, start a normal continuation immediately; for running
   sessions, accept a backend-owned reservation and let the existing exit
   monitor consume it after the current tool response/turn finishes.
4. Reap any warm executor server so the continuation creates a fresh process
   and performs MCP discovery from scratch.
5. Make duplicate calls idempotent and expose observable restart status.
6. Route clustered sessions through the persisted execution-to-worker owner.

## 4. Implement workspace restart orchestration

1. Define the workspace process-group boundary from existing execution process
   run reasons and ownership records; exclude server/worker replacement.
2. Stop workspace-owned processes through their authoritative local/worker
   container APIs while preserving dirty work through existing stop/recovery
   safeguards.
3. Restart persistent workspace processes as appropriate and resume the scoped
   coding-agent session through the same continuation path as session restart.
4. Persist/coordinate the operation independently from the HTTP request so a
   self-restart survives caller termination.
5. Add idempotency, partial-failure reporting, and unchanged worktree/Git-state
   verification.

## 5. Add backend and MCP surfaces

1. Add scoped backend routes for session restart/status and workspace
   restart/status, following existing workspace middleware and API envelopes.
2. Add `restart_session(session_id?, workspace_id?)` and
   `restart_workspace(workspace_id?, session_id?)` MCP tools with orchestrator
   context defaults and strict scope validation.
3. Register both tools in global and orchestrator routers and document response
   semantics, including the self-restart handoff.
4. Ensure errors contain actionable MCP-visible messages, not only typed data.

## 6. Bound runtime MCP discovery

1. Capture executor-owned discovery observations from the live protocol rather
   than treating settings probes as runtime truth.
2. Apply one monotonic deadline per server/discovery generation; repeated
   timeout or connection-close events append bounded observations but do not
   extend the deadline.
3. Transition every server to usable or a named terminal failure and record
   exact registered tool counts, including zero.
4. Return the terminal snapshots from restart results/status polling.
5. Preserve unknown fields when vendor protocols cannot prove a fact.

## 7. Make refresh support explicit

1. Keep Codex's verified live reload path.
2. Return immediate `unsupported` for executors without a published live MCP
   control; verify `CLAUDE_CODE` behavior explicitly in tests and documentation.
3. Remove any route/MCP wording that could imply an unsupported refresh was
   successfully queued.

## 8. Reconcile the UI with runtime registry state

1. Add an active-session MCP status projection alongside settings test state;
   label their distinct sources.
2. Render transport-connected/zero-tools, connecting, usable, and terminal
   failure states without collapsing them into one “connected” indicator.
3. Retain the last initialized snapshot during restart and show additive restart
   or reconnect progress.
4. Add restart actions and status feedback that use server-authoritative state.

## 9. Offer a prefilled Vibe Kanban issue

1. Build a pure, tested diagnostic bundle from safe runtime snapshots: server
   errors/timestamps, executor/workspace/session identity, healthy server tool
   counts, and discovery-attempt counts.
2. Extend the existing MCP Debug issue path and its Markdown-fence escaping,
   project routing, optimistic persistence, and cross-component deduplication.
3. Surface the action for terminal unusable state or registry/transport
   disagreement; never create an issue without the user's click.

## 10. Verify and document

1. Add unit and integration coverage for scoped defaults, cross-scope rejection,
   self-restart handoff, duplicate requests, idle/running sessions, clustered
   routing, deadlines, repeated transient errors, zero-tool registration,
   refresh support, diagnostics, and Git/worktree preservation.
2. Regenerate shared types through `pnpm run generate-types`.
3. Update MCP/tool documentation and project knowledge with exact shipped
   behavior and limitations.
4. Run formatting, focused Rust/frontend tests, generated-type checks, and the
   repository's broad check/lint suites.
5. Run independent Codex review, fix all confirmed significant findings, and
   repeat review and verification until clean.
6. Commit the knowledge base, open a pull request against the base branch,
   monitor CI, address failures, and merge it.
