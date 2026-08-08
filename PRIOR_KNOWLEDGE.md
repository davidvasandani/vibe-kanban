# Prior Knowledge: MCP-Driven Agent Restart

## Sources consulted

- `docs/knowledge-base/active-mcp-refresh.md`
- `wiki/agent-process-lifecycle.md`
- `docs/knowledge-base/clustered-workspace-execution.md`
- `docs/knowledge-base/codex-rollout-transfer.md`

## Relevant findings

### Live MCP refresh is not a cross-executor contract

The current refresh design is explicitly executor-owned and Codex-specific.
Codex’s reload acknowledgment means “queued,” not “adopted,” and the protocol
does not expose generation or process-restart evidence. Other executors have no
proven live reload. A cross-executor feature therefore must use Vibe Kanban’s
common execution lifecycle and a fresh process rather than expanding the live
reload abstraction.

### A turn and an OS process are currently coupled

One coding-agent turn maps to one `ExecutionProcess` and normally one process
lifetime. Codex, OpenCode, and ACP expose a turn-completion signal; most other
agents exit naturally. The exit monitor owns finalization, queued follow-up
dispatch, process-group reaping, and cleanup. Restart work must integrate with
that monitor rather than directly replacing child handles.

### Queued follow-up dispatch already solves the ordering boundary

The finalization path already claims and starts queued user follow-ups after a
turn, including an early-finalization path that must perform the same handoff.
This is the reusable mechanism for “finish the current turn, then continue.” A
new restart intent must not race or displace an actual user follow-up, and every
finalization shortcut must consume it consistently.

The existing queued-message service is process-local, but it is already the
authoritative handoff used for user-requested work after a running turn. Reusing
it preserves the established finalization ordering for this UI operation.

### Use the normal follow-up/resume path

Executor-specific `spawn_follow_up` implementations already preserve each
agent’s conversation identity using its supported resume mechanism. Starting a
fresh follow-up also rebuilds the launch environment and reads current executor
profile/MCP settings. This gives the desired behavior without inventing a
common “restart MCP child” protocol.

### Process ownership remains local

In clustered deployments, the coordinator owns SQLite/session authority while
the selected worker owns the agent process. Execution-to-worker affinity is
persisted and dispatch is idempotent by coordinator execution ID. A queued
restart must preserve affinity and use the normal coordinator dispatch path;
the coordinator must not attempt to kill or respawn worker-owned children
directly.

### Conversation transfer has stricter remote-worker constraints

Codex continuation on another worker requires verified rollout lineage. An MCP
restart should not implicitly migrate affinity. It should continue on the
current placement so existing follow-up behavior and rollout availability
remain valid.

## Design implications

1. Rename the user-visible operation from refresh/reload to restart and make it
   executor-neutral.
2. For idle sessions, create the continuation immediately through the existing
   follow-up action path.
3. For running sessions, leave the current process untouched and use the
   established queued-follow-up handoff.
4. Define deterministic precedence between a queued restart and queued user
   messages; never launch both concurrently.
5. Treat a fresh execution start as the success boundary. Do not use MCP status
   listing as proof.
6. Preserve current worker affinity and normal executor-specific conversation
   continuation.
