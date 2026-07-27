# Active MCP refresh

Contributing task: `8c27-refresh-mcp-tool`

Active-session MCP refresh is an executor capability, not an independent
connectivity test. VK queues the vendor's live reload operation, keeps the
request in `pending_next_turn`, and publishes one complete status snapshot only
after the next coding-agent turn has started. Executors without a proven live
reload contract report `unsupported`.

## Codex protocol boundary

The pinned Codex app-server protocol exposes `config/mcpServer/reload` and a
thread-scoped, paginated `mcpServerStatus/list`. Reload acknowledgement means
queued, not adopted. Status enumeration is the strongest next-turn proxy, but
the protocol does not expose an inventory generation ID, per-server
restart/reuse facts, or preservation of an old live connection when one
replacement fails. Keep those values unknown rather than inferring them.

## Lifecycle handoff

`SpawnedChild` carries an optional one-shot MCP control. Codex publishes it only
after initialize, thread registration, and `turn/start` succeed. The local
container registers the control by `(session_id, execution_id)` and removes it
only when that exact execution finishes, preventing an older cleanup task from
removing a newer control.

Pending state needs explicit terminal handling at every startup boundary:

- fail if a coding-agent process cannot spawn;
- fail if the control handoff closes before publication;
- fail as unsupported if the next coding-agent start has no MCP control;
- ignore setup, cleanup, archive, dev-server, and background-helper starts;
- if a request races an already-starting Codex execution, queue reload on that
  execution and defer confirmation to the following coding-agent turn.

Comparing `requested_at` with the execution row's `started_at` distinguishes an
idle request that the new turn may confirm from a request that arrived after
that turn had already resolved its configuration.

## State and safety

The coordinator is process-local and session-keyed. A write lock serializes
request, failure, and confirmation transitions; readers therefore see either
the previous complete server vector or the replacement vector. Duplicate
pending requests return retryable `busy` without advancing generation.

Public failures are allow-listed category/message/remediation tuples. Never pass
through executor errors, commands, environment values, authenticated URLs, or
raw subprocess output. Tool/resource counts remain optional, and the
last-successful timestamp advances only after a fully successful confirmation.
