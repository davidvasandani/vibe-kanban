# Implementation plan — issue comment delivery (vk/e252-deliver-issue-co)

See `SPEC.md` for the design and `PRIOR_KNOWLEDGE.md` for the constraints.

## Step 1 — Remote: list workspaces linked to an issue (read-only)

- Add `GET /v1/issues/{issue_id}/workspaces` to
  `crates/remote/src/routes/workspaces.rs`.
  - It is gated by `ensure_issue_access`.
  - It returns `ListIssueWorkspacesResponse { workspaces, archived_count }`
    in `api-types`.
  - It reuses `WorkspaceRepository::list_active_by_issue_id` and
    `count_by_issue_id`. These are existing cached queries, so no new
    `.sqlx` entries are needed.
- Add `RemoteClient::list_issue_workspaces`, `get_issue_comment` and
  `list_issue_comments`.

## Step 2 — Local DB: `issue_comment_deliveries`

- Migration `crates/db/migrations/2026101900000_issue_comment_deliveries.sql`.
  It has a unique `comment_id` and indexes on `(session_id, state)`,
  `issue_id` and `execution_process_id`.
- Model `crates/db/src/models/issue_comment_delivery.rs`:
  - `IssueCommentDelivery` (TS-exported), `DeliveryState`, `DeliveryReason`.
  - Functions: `insert_if_absent`, `find_by_comment_id`, `list_by_issue`,
    `list_by_session`, `find_by_execution_process_id`, `claim` (conditional
    update), `mark_delivered`, `mark_pending_retry`, `mark_terminal`,
    `reset_for_retry`, `list_due`, `list_stale_dispatching`.
- Add `ExecutionProcess::find_coding_agent_with_prompt_marker(session_id,
  marker)`. It is a LIKE over `executor_action` and is used for
  idempotent recovery.
- Run `pnpm run prepare-db`.

## Step 3 — Services: pure decision logic (unit-tested)

Add `crates/services/src/services/issue_comment_delivery.rs`:

- Agent state derivation: `derive_agent_state(latest_turn_status, running,
  queued)` returns `running | waiting | stopped | never_run`.
- Target selection: `select_target(candidates, explicit,
  origin_session) -> TargetDecision`. It covers the explicit, single
  running, single waiting, ambiguous, none and self cases.
- Loop guard: `hop_count_for(origin_hop) -> Result<i64, Blocked>`, with
  `MAX_AGENT_HOPS = 2`.
- Prompt composition: `compose_delivery_prompt(issue_key, comments)` adds
  the attribution header and the versioned marker `vk-issue-comment:<id>`.
  It also provides `extract_markers`.
- Retry backoff schedule: `next_attempt_delay(attempt)`.
- Add `ContainerService::turn_finalized_sender()`, which defaults to `None`.
  `finalize_task` emits `TurnFinalizedEvent{session_id, status}` for
  coding-agent turns. `LocalContainerService` implements it.

## Step 4 — Server: delivery service plus dispatcher

Add `crates/server/src/issue_comment_delivery.rs`:

- `resolve_candidates(deployment, issue_id)`. Remote linked workspaces are
  intersected with local workspaces and sessions, and agent state is
  computed for each session.
  - A remote failure is reported as `remote_unavailable`, never as "no
    target".
- `request_delivery(...)`:
  - Fetches the comment from the remote, which also authorizes the request.
  - Inserts or loads the row.
  - Allows re-resolution when the row is in a retryable state.
  - Dispatches the row.
- `create_comment(...)` covers the MCP path: idempotent remote create (a
  409 is resolved by fetch-and-verify), then `request_delivery` for replies.
- `dispatch_session(session_id, trigger)`:
  - Takes a per-session mutex and loads due pending rows.
  - Rows stay pending while a running agent or queued message exists.
  - Rows move to `no_target{turn_stopped}` when the last turn stopped.
  - On the sweeper path, it requires a 5 s grace after the last completion.
  - It claims rows and runs the marker recovery check.
  - It re-reads comments: deleted means cancelled, edited means the latest
    text is sent.
  - It calls the `routes::sessions::follow_up` handler directly. Rows are
    then marked delivered, or set to retry/fail.
- `spawn(deployment)`:
  - Subscribes to the turn-finalized event. Spawn it before boot resumes.
  - Runs a 15 s sweeper.
  - Runs startup recovery: stale `dispatching` rows are reconciled by the
    marker check.

## Step 5 — Server: routes

Add `crates/server/src/routes/issue_comment_deliveries.rs` and mount it in
the routes mod:

- `GET /api/issues/{issue_id}/agent-targets`
- `GET /api/issues/{issue_id}/comments`, which returns comments plus
  deliveries.
- `GET /api/issues/{issue_id}/comment-deliveries`
- `POST /api/issue-comment-deliveries`, which requests delivery for an
  existing comment. It also covers retry and target selection.
- `POST /api/issue-comments`, the create path (MCP).
- `GET /api/workspaces/{workspace_id}/issue-comments`, used by the
  conversation.

Add the new types to `generate_types.rs` and run `pnpm run generate-types`.

## Step 6 — MCP tools

Add `crates/mcp/src/task_server/tools/issue_comments.rs` and register it in
the tool router:

- `create_issue_comment`, where `intent` is required.
- `list_issue_comments`.
- `deliver_issue_comment`.
- Origin session: the MCP context's session id is sent as
  `origin_session_id`.
- Add a test for the response shape and the schema requirement on `intent`.

## Step 7 — Frontend

- `packages/web-core/src/shared/lib/api.ts`: add `issueCommentDeliveryApi`.
- Hook `useIssueCommentDelivery(issueId)`: React Query reads of targets and
  deliveries, polled while mounted.
- `IssueCommentsSectionContainer`:
  - Adds the composer intent toggle, defaulted from `default_session_id`.
  - On submit with Reply, awaits `persisted` and then calls the request API.
  - Adds per-comment delivery chips with Retry and Choose session actions.
- `packages/ui/src/components/IssueCommentsSection.tsx`: presentational slots
  `renderComposerControls`, plus `renderCommentFooter(commentId)`.
- Conversation:
  - `useWorkspaceIssueComments(workspaceId)`.
  - `deriveConversationEntries` gains an optional `issueComments` input.
    - A user message of an execution with deliveries becomes an
      `issue_comment` card.
    - Undelivered comments are merged chronologically, within the loaded
      window only.
  - `DisplayConversationEntry` renders an `IssueCommentEntry` with an
    indicator, the author, the comment's own time and a link to the issue.
- i18n keys in all locales. Vitest for the merge/dedupe derivation and for
  default intent selection.

## Step 8 — Tests

- `services` unit tests cover:
  - Target selection: ambiguous, none, archived, terminal issue, explicit
    not linked, self, loop guard.
  - Prompt marker round-trip and backoff.
- `db` tests: duplicate insert idempotency, claim exclusivity, retry reset.
- `server` tests drive the dispatcher with an in-memory SQLite and a fake
  starter/remote seam:
  - restart recovery: a `dispatching` row with an existing marker execution
    becomes delivered without a second start;
  - backend failure leads to retries and then `failed`;
  - a running target stays pending until finalize;
  - a stopped turn is not auto-started;
  - an edit before dispatch sends the latest text, and a deletion cancels;
  - a duplicate request returns the same row;
  - an authorization failure (remote 403) creates no row.
- Frontend Vitest for the derivations.

## Step 9 — Verify

- Run `pnpm run format`, `pnpm run check`, `pnpm run lint`,
  `cargo test --workspace` (with targeted crates first) and
  `generate-types:check`.
- Do a real VK end-to-end run where the environment allows it. Otherwise
  record the blocker honestly.
