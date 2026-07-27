# Active Workspace MCP Tool Refresh

## Summary

Vibe Kanban must let an active workspace session reconcile its configured MCP
servers and publish a refreshed capability inventory without replacing the
workspace, agent session, or conversation. Refresh is an explicit operation
available from the workspace UI and from the Vibe Kanban API/MCP surface.

The refresh boundary is the live coding-agent session. Vibe Kanban coordinates
the request, but each supported executor remains responsible for the protocol it
uses to reload MCP configuration and capabilities. Executors that cannot reload
in place must return an explicit unsupported result rather than claiming
success.

## Goals

- Add a user-visible **Refresh MCP tools** action to active workspace sessions.
- Expose refresh through an authenticated/local API and an orchestrator-scoped
  Vibe Kanban MCP tool.
- Re-read the active executor's MCP configuration and reconcile additions,
  removals, enables, disables, configuration changes, and credential changes.
- Preserve healthy unchanged connections when the executor supports reuse.
- Restart or reconnect changed servers, initialize them, and enumerate tools,
  resources, and prompts before publishing a new inventory.
- Make the new complete inventory visible on the next turn while preserving
  workspace state and conversation history.
- Keep the last known-good inventory for a server whose refresh fails.
- Provide safe, classified, per-server diagnostics and useful refresh metadata.

## Non-goals

- Defining a new MCP protocol-level refresh extension for arbitrary third-party
  clients.
- Silently restarting an entire coding-agent conversation when an executor has
  no in-session MCP reload mechanism.
- Returning configuration secrets or raw subprocess/network diagnostics to the
  browser, agent, or logs.

## User Experience

An active workspace session exposes a **Refresh MCP tools** action near its
session controls. While refresh is running, the action shows progress and cannot
start a duplicate request. The result shows:

- overall outcome: refreshed, partially refreshed, busy, unsupported, or failed;
- last successful refresh time;
- configured server identifier;
- per-server status and discovered tool count;
- whether the connection was reused, restarted, added, removed, or disabled;
- safe remediation for any failure.

The UI must not display success until the executor confirms that the replacement
inventory has been published. A busy response is retryable and identifies
whether another refresh or an active MCP call blocked the operation.

## Functional Design

### Refresh request

The server accepts a refresh request for one active coding-agent session. It
validates that the session belongs to the workspace in scope and is running,
then delegates to the execution backend that owns the live agent process.

The same service operation backs:

1. the workspace-session REST endpoint;
2. the web action; and
3. an orchestrator-scoped `refresh_mcp_tools` VK MCP tool.

The MCP tool defaults to its scoped workspace and orchestrator session. Global
mode requires explicit identifiers.

### Executor capability

The executor interface gains a typed MCP-refresh capability. A refresh adapter
must:

1. serialize with other refreshes for the session;
2. wait behind an in-flight MCP call when safely supported, or return a
   retryable busy result;
3. re-read the executor-native MCP configuration;
4. compare secret-safe configuration fingerprints by configured server ID;
5. reuse unchanged healthy servers;
6. stop/reconnect changed or removed servers only when safe;
7. initialize affected servers and list tools, resources, and prompts;
8. validate returned capability schemas;
9. construct a complete candidate inventory; and
10. atomically publish the candidate inventory for subsequent turns.

If an executor's native CLI/API does not expose live reload, its adapter reports
`unsupported` with remediation. It must not restart the conversation and label
that as an in-place refresh.

### Reconciliation

Server definitions are normalized before comparison. The comparison includes
transport kind, command/package and arguments, URL, enabled state, relevant
headers/environment by keyed digest, and other transport settings. Raw secret
values never appear in the fingerprint report.

Each configured server is classified as:

- `unchanged_reused`;
- `added`;
- `changed_restarted`;
- `removed`;
- `disabled`;
- `refreshed` (reinitialized without process restart);
- `failed_retained` (refresh failed; last known-good capabilities remain); or
- `failed_unavailable` (no last known-good capabilities exist).

Removed or explicitly disabled servers are absent from the new inventory after a
successful atomic publish. Failed changed servers retain their prior inventory,
if any, until a later successful refresh or explicit disable.

### Atomicity and concurrency

Each live session has one refresh coordinator guarded by a non-reentrant lock and
generation number. Tool dispatch reads one immutable inventory snapshot. Refresh
builds a candidate snapshot off to the side and swaps it only after all server
outcomes have settled. Dispatch therefore sees the entire old or entire new
generation.

An in-flight call retains the connection/snapshot generation it started with.
Affected connections are retired only after active calls release them. If an
executor cannot safely defer retirement, refresh returns `busy_active_call`.
Concurrent refresh requests return `busy_refresh_in_progress`, including a
retryable marker.

### Partial failure

A single-server failure does not prevent healthy additions, removals, or updates
from being published. For the failing server, the candidate snapshot uses its
last known-good capability set when available. Overall status is
`partially_refreshed`. If validation or publication of the complete snapshot
itself fails, no swap occurs and the previous generation remains active.

## API Contract

Suggested REST shape:

`POST /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/refresh`

Response fields:

- `status`;
- `retryable`;
- `generation_before` and `generation_after`;
- `started_at`, `completed_at`, and `last_successful_refresh_at`;
- `servers[]` with `server_id`, `status`, capability counts, `restarted`, and an
  optional structured error;
- a safe summary.

Structured errors contain `code`, `category`, `message`, `remediation`, and
`retryable`. Categories:

- `executable_unavailable`;
- `process_launch_failed`;
- `initialize_failed`;
- `authentication_failed`;
- `capability_list_failed`;
- `invalid_capability_schema`;
- `timeout`;
- `busy_refresh_in_progress`;
- `busy_active_call`;
- `unsupported`;
- `internal`.

The endpoint uses the project's normal API envelope and status conventions.
Busy responses use a conflict/locked-style HTTP status; missing or inactive
sessions use the existing not-found/conflict conventions.

## Security and Redaction

All errors pass through a dedicated MCP refresh sanitizer before logging or
serialization. It removes or replaces:

- environment values and tokens;
- Authorization, Cookie, and OAuth material;
- credentials embedded in URLs;
- raw authenticated URLs and query strings;
- command arguments identified as secret-bearing;
- subprocess output that cannot be proven safe.

Public diagnostics identify only the configured server ID, error category, safe
message, and remediation. Configuration fingerprints are keyed digests and are
never returned to clients.

## Observability

Emit structured, secret-safe events for refresh start, per-server outcome,
inventory publication, busy rejection, and completion. Metrics should cover
duration, status, executor, transport, restarts/reuses, tool-count delta, and
failure category. Persist or retain sufficient session metadata to render the
last successful refresh and latest per-server status.

## Compatibility

The feature is capability-gated per executor. Existing session launch and MCP
configuration behavior remain unchanged. API clients can distinguish supported,
unsupported, busy, partial, and complete outcomes without parsing prose.

## Test Requirements

Automated tests must cover:

- stdio tool addition and removal;
- streamable-HTTP tool addition and removal;
- adding, removing, enabling, and disabling a server;
- unchanged connection reuse and changed connection restart;
- credential renewal after authentication failure;
- partial server failure with last known-good retention;
- timeout and malformed `tools/list`;
- initialize/handshake and launch failures;
- refresh during an in-flight call;
- concurrent refresh attempts;
- atomic snapshot visibility;
- removed-tool unavailable behavior;
- error/log secret redaction;
- REST and VK MCP authorization/scope;
- UI status, counts, restart indicator, and no false-success state; and
- the Slack `v1.3.0-vk.2` regression, proving `attachment_get_data` appears in
  the same workspace conversation after refresh.

## Acceptance

Starting with tool set A, changing a configured MCP server to expose A+B, and
refreshing the active session makes B callable on the next turn without creating
a workspace or losing history. The inverse removes B cleanly. Healthy servers
remain usable through partial failures, failed servers retain last known-good
tools unless disabled, and no intermediate inventory or secret material is
observable.

## Open Technical Questions

- Which current executor CLIs expose a supported live MCP reload command or
  control channel, and what guarantees does each provide?
- Where does each executor hold its live tool inventory, and can VK observe the
  generation that becomes active?
- Should refresh-status metadata be persisted in the database or retained only
  with the live execution process?
- Can all supported executors separately enumerate resources/prompts, or should
  those counts be optional capability fields?
