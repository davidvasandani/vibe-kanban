# Data Model: Active MCP Refresh

All state is process-local and keyed by `session_id`.

## `McpRefreshState`

- `generation: u64` — most recently requested generation.
- `phase: idle | queued | confirming`.
- `requested_at: DateTime<Utc>`.
- `last_successful_refresh_at: Option<DateTime<Utc>>`.
- `requested_by_execution_id: Option<Uuid>`.
- `previous_servers: map<server_id, McpServerRefreshSnapshot>`.
- `latest_servers: map<server_id, McpServerRefreshSnapshot>`.

Only one queued/confirming generation exists per session. State is removed on
session/workspace teardown and server shutdown.

## `McpServerRefreshSnapshot`

- `server_id: String`.
- `status: ready | failed_retained | failed_unavailable | removed | disabled`.
- `tool_count: Option<u32>`.
- `resource_count: Option<u32>`.
- `prompt_count: Option<u32>`.
- `restart_occurred: true | false | unknown`.
- `error: Option<McpRefreshError>`.

No server definition, command, environment, header, URL, tool arguments, or raw
diagnostic is retained.

## `McpRefreshError`

- `category`: stable enum from the feature spec.
- `message`: bounded safe message.
- `remediation`: safe category-specific guidance.
- `retryable: bool`.

## State transitions

`idle -> queued` when the request is accepted. A second request in `queued` or
`confirming` returns busy without changing generation.

`queued -> confirming` when the next Codex execution registers its live control
handle and begins/has begun the active turn.

`confirming -> idle` after the complete paginated status inventory is collected
and atomically installed. Successful confirmation advances the last-success
timestamp. Partial status failure retains the previous snapshot for the failed
server. Whole-confirmation failure leaves the previous snapshot and queued
generation intact for a bounded retry or records a safe failure according to the
coordinator policy.

Unsupported executors do not create state.
