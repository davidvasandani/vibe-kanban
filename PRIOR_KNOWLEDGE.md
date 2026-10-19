# Prior knowledge — vk/e252-deliver-issue-co

This file distils pages from the knowledge base (`wiki/`, 56 pages) that are
relevant to delivering issue comments to workspace sessions. The knowledge
base was not modified; it was only read.

## Directly relevant

- **issue-workspace-lifecycle** (`vk/e464`): "comment-driven activation"
  there means *workspace conversation messages*. The clarify stage of that
  task explicitly excluded issue-level comments, so this task fills that gap.
  Reusable rules:
  - Route accepted activity through `ContainerService::activate_workspace`.
  - Publish queued work **before** awaiting remote I/O, because a turn can
    finish while the request is in flight.
  - Never put remote HTTP latency ahead of queue insertion or dispatch.
  - SQLite and Postgres are separate stores. There is no distributed
    transaction.
- **auto-error-remediation**:
  - `finalize_task` is the one convergence point for every exit path,
    including the local exit monitor, skipped-cleanup and worker
    reconciliation.
  - Emit from it with a non-blocking `broadcast::send` through a trait
    accessor that defaults to `None` (`error_remediation_sender`). Subscribe
    **before** boot resumes interrupted agents.
  - Server-side consumers may call route handlers directly (Axum extractors
    are tuple structs).
  - Reserve before I/O and fail closed.
  - Use a durable recursion guard; in-memory guards do not survive restarts.
  - `RemoteClient::create_issue_comment` already exists; remediation uses it.
- **worker-execution-hardening / 9f36 queued-message spec**:
  - The queued follow-up is in memory and holds one message per session.
  - It is **dropped after any failed/killed/interrupted turn** (existing
    policy), and it is not persisted across restarts.
  - The no-change cleanup-skip path must dispatch queued work before
    finalizing.
- **session-chat-history-search**:
  - A session's turns are its non-dropped `CodingAgent` executions.
  - Keep "could not read" distinct from "read, nothing matched" for any
    aggregate reader. This also applies to remote lookups during target
    resolution: a failed lookup is not "no target".
- **kanban-items-state-and-activity-grouping**: the UI's "Waiting for feedback"
  group means *not running and no pending approval*. It is derived and never
  persisted, and issue status is not agent state. This matches the
  task's requirement that "In progress" is not evidence of a running agent.
- **kanban-issue-panel-sections**:
  - The comments section is a render-prop slot in `KanbanIssuePanel`.
  - Component tests live in `packages/remote-web/src/test/*.test.tsx` and
    must run with `NODE_ENV=test`.
  - Without an i18n provider, `t()` returns raw keys.
- **read-only-markdown-links**: conversation and comment markdown is rendered
  read-only. Composers are disabled only while they are submitting.
- **host-env-sensitive-tests**: some Slack/GitHub-routing tests fail only
  because of host env vars. Re-run with those vars unset before blaming a
  diff. Run tests in the foreground.

## Constitution principles that bind this design

- **V:** remote mutations are transactional and covered by a txid.
  - New remote routes stay read-only here, so no new mutation is needed.
  - Terminal statuses are identified by name, case-insensitively
    (`is_active_status_name`).
- **XII:** asynchronous handoffs have one owner. A claim boundary is needed
  against authoritative state, without holding locks across awaits, and
  tests must cover both orderings.
- **XVIII/XXII:** retries are idempotent and can never create a second
  execution. A missing response is not evidence of success.
- **XX (time):** a derived row inherits its parent event's timestamp. An
  issue-comment card shows the comment's `created_at`, not the turn's.
- **XXI:** reuse existing resolvers (`is_active_status_name`, MCP
  `issue_url`, `follow_up` executor checks) instead of re-deriving them.
  Failures name what failed.

## Gaps confirmed by investigation (not in the knowledge base)

- There is no comment→session routing, no local comment proxy, no MCP
  comment tools, and no comment `source` field.
- There is no persisted "waiting for input" state. Agent state must be
  derived from execution status.
