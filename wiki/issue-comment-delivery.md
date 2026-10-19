# Issue comment delivery to workspace sessions

How an issue comment reaches the agent in a linked workspace session, and the
traps found while building it (`vk/e252-deliver-issue-co`).

## Why comments did not reach agents

Issue comments and workspace conversations are separate stores, and nothing
connected them:

- Comments are remote Postgres rows (`issue_comments`), written by the UI
  through Electric.
- Conversations are local SQLite executions.

The remote comment create had two side effects only: analytics and
`notify_issue_subscribers`. The local server could only *create* comments
(for error remediation), and the MCP had no comment tools. Nothing was
broken; the routing simply never existed.

## Shape

- **One path for every surface.**
  - `server::issue_comment_delivery` owns delivery.
  - The UI keeps its optimistic Electric insert and then calls
    `POST /api/issue-comment-deliveries` with the comment id.
  - The MCP calls `POST /api/issue-comments`, which creates the comment and
    delivers it.
  - Both re-read the comment from the remote with the user's token. That
    read is both the authorization check and the authoritative text.
- **Explicit intent.** Every comment is `note` or `reply_to_agent`.
  - The MCP schema makes `intent` required, with no default.
  - The UI defaults to reply only when `agent-targets` names exactly one
    waiting or running session.
  - Intent is never inferred from text.
- **One target.** The target is an explicit session, the only session that
  has run a turn, or the only running one. Anything else is
  `needs_target`; nothing is broadcast.
  - Implicit delivery never resumes a stopped or failed turn. An explicit
    choice is consent to do so.
  - Archived workspaces and Done/Cancelled issues are never resumed.
- **Durable, at most once.** `issue_comment_deliveries` has a unique
  `comment_id`.
  - **Resolve before insert.** A crash mid-resolution leaves no row, rather
    than a pending row with no session that nothing would ever pick up.
  - **Claim, snapshot, start.** A conditional update claims the row. The
    exact text is persisted before the turn starts, so recovery records what
    the agent actually received, not a later edit.

## The turn boundary is recorded, never guessed

A comment for a busy session waits for its turn boundary. A time-based grace
(first 5 s, then 2 min) is wrong in both directions. The exit monitor
records completion, then commits, runs cleanup and hands off the queued
message, and none of that is bounded. After `take_queued`, the queue is
empty and the next execution row does not exist yet. A dispatcher that looks
then sees an idle session and starts a competing turn. Codex found this in
review rounds 1 and 2.

The fix is a durable boundary:

- `ContainerService::finalize_task` is reached only when no queued follow-up
  started in place of the chain. It writes `session_turn_boundaries` for the
  session's latest coding-agent turn **synchronously**, before any early
  return, and only then emits the wake-up event.
- The dispatcher may start a turn only when that row names the current
  latest turn, or when the turn ended before this process started. No exit
  monitor or in-memory queue survives a restart.
- Do not identify the turn in the asynchronous event handler. A delayed
  event for turn A would "confirm" turn B.

A one-line busy check also needs these: a running coding agent in the
session, a queued message (`QueuedMessageService::has_queued`), and any
non-persistent process in the workspace (setup or cleanup scripts).

## An execution row is not evidence of launch

`start_execution` writes the execution row before it dispatches and keeps it
when the launch fails, for example when the executable is missing or the
worker is unavailable. A crash between the two leaves a `Running` row that
boot marks `Interrupted`. So a prompt marker in an execution row proves
nothing on its own.

Evidence is durable launch evidence from the same query:

- `pgid IS NOT NULL` (never cleared);
- an `execution_worker_jobs.accepted_at`;
- or `coding_agent_turns.agent_session_id`.

A marked turn that launched and then failed counts as delivered and is never
replayed. When the latest turn is our own unlaunched attempt for the batch,
the agent did not stop, and that attempt counts as the boundary. Without
that rule, a failed launch would turn its own retry into "turn stopped".

## Markers

The prompt is attributed user content: a header naming author, issue and
comment id, followed by the quoted message. A versioned marker line
`<!-- vk-issue-comment:v1:<id> -->` is used only for evidence.

- It must start a line. Quoted text starts with `> `, so a comment cannot
  forge another comment's marker.
- In stored JSON the preceding newline is the two characters `\n`;
  `stored_marker_needle` encodes that.

## Loop guards

- An agent's comment never targets its own session. The MCP sends
  `origin_session_id` / `origin_workspace_id` from its context; the
  workspace's running agent is used when only the workspace is known.
- `hop_count` is one past the delivery that started the origin's current
  turn. Above 2 the delivery is `blocked{loop_guard}`.
- The comment tools are global-mode only. Orchestrator mode deliberately has
  no remote-issue tools, and a pinned test enforces that.

## Frontend

- **Conversation rows.** A delivered comment replaces the user-message row
  of the turn it started, with no second copy. Other linked comments become
  `system_message` rows tagged `issue-comment:<id>`. A synthetic
  `user_message` would split turns and collapse the real last turn's
  thinking.
  - Rows are placed before the first turn that started after the comment.
  - They are dropped when older than a partially loaded history window.
- **Context updates.** Publish the comments context only when the data or
  the placement keys change. `flushPendingUpdate` runs every streamed frame,
  and a fresh context value would re-render every entry.
- **Polled reads.** These reads carry a deadline (`withDeadline`, XLIII).
  A host that cannot answer shows "delivery status unavailable", never
  "delivered".

## Residual

A user's own follow-up submitted at the same instant as a dispatch is not
serialized with it. This race already exists between any two follow-ups.

## Contributed by

- `vk/e252-deliver-issue-co`
