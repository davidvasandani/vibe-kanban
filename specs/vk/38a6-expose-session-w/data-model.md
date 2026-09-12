# Data Model: MCP Recovery

## `McpRecoveryResult`

- `generation: u64` — monotonically increases per session/workspace target.
- `scope: session | workspace`
- `workspace_id: UUID`
- `session_id: UUID?` — requesting/resumed session for workspace recovery.
- `status: accepted | in_progress | completed | partially_completed | failed`
- `requested_at: timestamp`
- `deadline_at: timestamp`
- `completed_at: timestamp?`
- `executor: string`
- `servers: McpRuntimeServerStatus[]`
- `processes: WorkspaceProcessRestartStatus[]` — workspace scope only.
- `error: SafeRecoveryError?`

## `McpRuntimeServerStatus`

- `server_id: string` — stable native configuration identifier.
- `state: connecting | ready | connected_no_tools | failed`
- `tool_count: integer?`
- `resource_count: integer?`
- `prompt_count: integer?`
- `discovery_attempts: integer`
- `first_observed_at: timestamp`
- `last_observed_at: timestamp`
- `terminal_at: timestamp?`
- `error: SafeRecoveryError?`
- `events: McpDiscoveryEvent[]` — bounded newest observations.

## `McpDiscoveryEvent`

- `code: connect_timeout | connection_closed | authentication_failed |
  initialize_failed | capability_list_failed | unsupported | internal`
- `observed_at: timestamp`

Messages are derived from allow-listed codes; raw subprocess output is not part
of this model.

## `WorkspaceProcessRestartStatus`

- `execution_id: UUID`
- `run_reason: string`
- `status: stopped | restarted | not_restartable | failed`
- `replacement_execution_id: UUID?`
- `error: SafeRecoveryError?`

## State transitions

```text
accepted -> in_progress -> completed
                        -> partially_completed
                        -> failed

connecting -> ready
           -> connected_no_tools
           -> failed
```

Transitions are monotonic within a generation. A duplicate request returns the
active generation instead of creating another.

## Existing durable relationships

- `Session.workspace_id` validates ownership.
- `ExecutionProcess.session_id`, `run_reason`, and worker-affinity records locate
  the process owner.
- `QueuedMessage.restart_agent` and its reservation identify the continuation
  handoff.
- Execution history, normalized transcript, worktree paths, and Git repositories
  are retained; recovery results reference them and do not replace them.

