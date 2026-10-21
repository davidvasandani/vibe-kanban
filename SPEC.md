# Issue comment delivery to linked workspace sessions

Task: `vk/e252-deliver-issue-co`

## Problem and evidence

On VAS-859 an agent stopped at specification clarification, waiting for a
device address and secret references. Those details were posted as an issue
comment. The comment was saved (still visible after a fresh navigation), but
the linked workspace conversation never received it and the agent never
resumed. A separate `run_session_prompt` call was needed.

- Issue `cee841f2-4c0d-4518-9d0a-3ff993e6d93d`, workspace
  `87d37e45-9885-4587-a62b-8005513e8399`, session
  `e6618fc8-feb4-40fc-a44d-3d55e19a4dc3`.
- Waiting execution `d4078149-…` completed 04:28 UTC; comment posted ≈04:45;
  manual follow-up `3bd95943-…` started 04:48.
- The VK MCP had no issue-comment create/read tools; the comment needed a
  browser.

## Root cause (investigated)

This is **missing routing between two intentionally separate models**, not a
broken subscription:

- Issue comments live only in the remote Postgres `issue_comments` table
  (`crates/remote/src/db/issue_comments.rs`). The UI writes them through the
  Electric mutation (`useIssueContext().insertComment`). Create has two side
  effects only: analytics and `notify_issue_subscribers`.
- Workspace conversations are local SQLite `execution_processes` per
  `session`. A turn's user message is the prompt in its `executor_action`.
- No code on the local server, in services, local-deployment or MCP reacts to
  comments. The local `RemoteClient` can only *create* comments (used by
  auto error remediation). There is no local proxy route and no MCP tool.
- There is no persisted "waiting for input" state.
  `ExecutionProcessStatus` is
  `Running|Completed|Failed|Killed|Interrupted|Indeterminate`. "Waiting"
  can only mean that the session's latest coding-agent turn ended
  `Completed` and nothing runs now.
- The follow-up queue (`QueuedMessageService`) is in memory, holds one
  message per session (a new one replaces the old), and is lost on restart.
  The exit monitor consumes it at a turn boundary. It is not suitable as
  durable delivery storage.

## Design

### Delivery record (local SQLite, authoritative for delivery)

The new table `issue_comment_deliveries` is keyed **uniquely by
`comment_id`**. Each comment gets at most one delivery to an agent, so a
repeated client request, a retry or recovery cannot duplicate a delivery.
It records:

- **Comment details:** `issue_id`, `comment_id`, author id, the comment's
  `created_at`, and the delivered message snapshot.
- **Origin:** `source` (`ui` | `mcp`), `origin_session_id`, `hop_count`.
- **Target:** `workspace_id`, `session_id`, `execution_process_id`.
- **Status:** `state`, a `reason` code, `error`, `attempt_count`,
  `next_attempt_at`, and the timestamps (`delivered_at`).

States:

| state | meaning | user action |
|---|---|---|
| `pending` | accepted; target busy (agent running / queued message) or retry scheduled | none (auto) |
| `dispatching` | claimed by the dispatcher | none |
| `delivered` | follow-up execution started (`execution_process_id`) | open conversation |
| `needs_target` | several eligible sessions, none established | choose target |
| `no_target` | no eligible session (reason: no linked workspace, other host, archived, issue Done/Cancelled, last turn failed/stopped…) | retry / choose target |
| `failed` | dispatch error after automatic retries | retry |
| `blocked` | loop guard or self-delivery | none |
| `cancelled` | comment deleted before dispatch | none |

### Shared delivery path

`server::issue_comment_delivery` owns one service used by the UI and MCP:

- `request_delivery(comment_id, issue_id, target_session_id?, source,
  origin_session_id?)`
  - Fetches the comment **from the remote** using the signed-in user's
    credentials. That fetch is the authorization check, and remote content
    is authoritative over client-supplied text.
  - Inserts the delivery idempotently.
  - Resolves the target and dispatches.
- `create_comment(issue_id, message, parent_id?, intent, target?, id?)`
  (MCP path)
  - Creates the remote comment with a client id. A 409 on retry is resolved
    by fetching and verifying the existing comment, so retries are idempotent.
  - When `intent = reply_to_agent`, calls `request_delivery`.
- The UI keeps its optimistic Electric insert. With **Reply to agent**
  selected, it awaits `persisted` and then calls `POST
  /api/issue-comment-deliveries`. A failed call shows "not delivered —
  Retry" rather than success.

### Target resolution (deterministic, no text matching)

1. Candidates are the remote workspaces linked to the issue (new remote
   route `GET /v1/issues/{issue_id}/workspaces`, gated by
   `ensure_issue_access`) whose `local_workspace_id` exists on this host and
   is not archived locally.
2. The issue's status must be active (`is_active_status_name`, the existing
   rule). Done/Cancelled issues give `no_target{issue_terminal}`. Issue
   status is never used as evidence that an agent runs.
3. Sessions are the candidate workspaces' sessions that have at least one
   coding-agent turn.
   - Agent state comes from executions: `running` (a coding agent is running
     or a message is queued), `waiting` (latest turn `Completed`),
     `stopped` (latest turn `Failed`/`Killed`/`Interrupted`/
     `Indeterminate`), `never_run`.
4. An explicit `target_session_id` must belong to a candidate. Otherwise the
   result is an authorization-style rejection, `no_target{target_not_linked}`.
5. Without an explicit target:
   - 0 sessions → `no_target`.
   - One running session (or one waiting session) is the "established active
     target".
   - Otherwise → `needs_target`, with the candidate list.
   - Nothing is ever broadcast.
6. Loop guards:
   - `origin_session_id == target` → `blocked{self}`.
   - `hop_count` = the origin execution's delivery hop + 1. Above
     `MAX_AGENT_HOPS = 2` → `blocked{loop_guard}`. Human UI comments have hop 0.

### Dispatch

(This section was revised after three rounds of independent review.)

- **Serialization.** A per-session async mutex serializes dispatch on one
  host and is held until the turn has started. A row is claimed with a
  conditional update (`pending → dispatching`).
- **Busy target.** A running agent, a queued message, or a running
  setup/cleanup script keeps the delivery `pending`.
- **Turn boundary, recorded, never guessed.**
  - `ContainerService::finalize_task` writes `session_turn_boundaries` for
    the session's latest turn synchronously, before any early return.
    `finalize_task` runs only when no queued follow-up was started.
  - It then emits `turn_finalized_sender()` as a wake-up.
  - A new turn may start only when that row names the current latest turn,
    or when the turn ended before startup reconciliation finished.
  - Elapsed time never authorizes a dispatch, so the exit monitor's
    queued-message handoff is never raced.
- **Stopped turn.** Implicit delivery to a failed/killed turn becomes
  `no_target{turn_stopped}`. An explicit choice may resume it.
- **Idle target.**
  - **Evidence check.** An earlier attempt counts as delivered only if the
    agent was **launched**: `pgid`, an accepted worker job, or the vendor
    agent's session id. An execution row alone is not evidence.
    - A worker that never confirmed acceptance is `failed{launch_uncertain}`.
      It is not resent automatically.
    - The latest turn being our own unlaunched attempt counts as a boundary,
      not a stopped agent.
  - **Re-check before launch.** The comment is re-read, and the issue status
    and the workspace's remote link are re-checked.
  - **Snapshot, then start.** The exact text is persisted before launch. The
    follow-up then starts through `routes::sessions::follow_up`.
  - **Batching.** Pending comments are batched into one prompt.
- **Prompt framing.** Attributed user content: a header plus quoted text,
  with a line-start marker `<!-- vk-issue-comment:v1:<id> -->`.
- **Errors.** Two automatic retries with backoff (15 s, 60 s), then
  `failed`. A manual retry resets the attempts.
- **Durability.**
  - Rows are resolved before insert, so no row waits without a target.
  - A 15 s sweep and startup recovery resolve `dispatching` rows from launch
    evidence.

### Edits and deletions

- Delivered text is a snapshot. Editing never redelivers. Both surfaces show
  "edited after delivery — agent received the original".
- A pending delivery re-reads the comment when it dispatches, so the latest
  text is sent because nothing was delivered yet. A comment deleted before
  dispatch → `cancelled`.
- A comment deleted after delivery keeps its delivery record. The
  conversation card renders the snapshot labelled "deleted from issue".

### API surface (local server)

- `GET /api/issues/{issue_id}/agent-targets` → candidate workspaces/sessions
  with agent state, `default_session_id` (only when exactly one session is
  eligible and waiting/running), and `issue_active`.
- `GET /api/issues/{issue_id}/comment-deliveries` → delivery records.
- `POST /api/issue-comment-deliveries` `{comment_id, issue_id,
  target_session_id?}` → delivery record (idempotent; retry/select target on
  an existing `failed`/`no_target`/`needs_target` row re-runs resolution).
- `POST /api/issue-comments` (MCP create) → `{comment, issue_url, delivery?}`.
- `GET /api/issues/{issue_id}/comments` → remote comments plus deliveries.
- `GET /api/workspaces/{id}/issue-comments` → the linked issue plus comments
  plus deliveries, for the conversation.

### MCP tools

- `create_issue_comment{issue_id, message, intent: note|reply_to_agent
  (required), target_session_id?, parent_id?, comment_id?}` returns:
  - `comment_id`, `issue_url`, `saved_on_issue: true`
  - `delivery: {state, reason, workspace_id, session_id,
    execution_id, candidates}` (or `not_requested` for notes)
- `list_issue_comments{issue_id}` returns comments with author, timestamps
  and delivery state.
- `deliver_issue_comment{comment_id, issue_id, target_session_id?}` retries
  or selects the target for an existing comment.
- Origin session is taken from the MCP's resolved context (loop guard).

### UI

- **Issue panel composer:** a two-option toggle, "Note" and "Reply to
  agent".
  - It shows the target name and agent state.
  - It defaults to Reply only when `default_session_id` exists, and to Note
    otherwise.
  - Reply is disabled with a reason when nothing is eligible.
- **Each comment:** shows a delivery chip with Retry / Choose session
  actions:
  - Saved on issue · Not sent to agent
  - Queued for <session> (agent running)
  - Delivered · resumed <session>
  - Needs target
  - No eligible session (reason)
  - Delivery failed
  - Edited after delivery
- **Workspace conversation:**
  - A turn whose execution came from a delivery renders its user message as
    an **issue-comment card**. The card shows an issue-comment indicator,
    the author and the comment's own timestamp, and links back to the issue
    comment (`/projects/{p}/issues/{i}`).
  - The raw framed prompt is not shown twice.
  - Linked comments not delivered to this session render as compact cards
    placed chronologically between turns.
- **Remote-web, or no local host:** the delivery calls fail closed. The
  comment shows "delivery status unavailable" and never "delivered".

## Acceptance mapping

- **AC1/AC2:** UI reply and MCP `create_issue_comment` both resume the single
  waiting session. Each returns the comment id and execution id.
- **AC3:** a note never creates a delivery. Done/Cancelled issues, archived
  workspaces and stopped turns never auto-start.
- **AC4:** a running target stays `pending` until finalize, then exactly one
  execution starts. Batching means one message.
- **AC5:** dispatcher unit tests cover ambiguous, none, archived,
  completed/cancelled, authorization (remote 403/404), backend failure,
  restart recovery (dispatching plus marker), duplicate requests (unique
  `comment_id`) and edits (snapshot).
- **AC6/AC7:** both surfaces read the persisted delivery records and expose
  Retry/Choose-target actions.
- **AC8:** real VK verification, independent review, pipeline, PR, merge.

## Out of scope

- Steering inside a running turn (vendor stdin injection).
- Delivering to workspaces on other hosts.
- Changes to the remote comment schema.
- Hosting changes.
