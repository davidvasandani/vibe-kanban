# Data Model: Active MCP Inventory Refresh

No database migration is planned. The feature uses existing session,
execution, queue, and refresh state.

## Durable entities

### Session

- identifies the logical conversation and owning workspace;
- survives the agent-process restart;
- subsequent follow-ups retain the session while external executor state may be
  freshly started or resumed according to the normal executor contract.

### ExecutionProcess

- identifies one coding-agent process/turn;
- its exact `running` state drives restart confirmation and queue ownership;
- a restart produces a later coding-agent execution rather than mutating the
  active process in place.

### Executor profile/native MCP configuration

- the current settings-owned source for assigned MCP definitions;
- a fresh agent process reads the current version, not the earlier execution's
  connector-test snapshot.

## Process-local state

### QueuedMessage restart fields

- `restart_reservation`: request-owned claim hidden from finalization;
- `restart_agent`: committed handoff marker;
- `queued_at`: exact generation/claim identity;
- `data`: synthetic continuation plus the selected executor configuration.

### McpRefreshResult

- `generation`, `requested_at`, `status`, and optional success timestamp;
- complete per-server snapshot keyed by stable `server_id`;
- counts and `restart_occurred` remain optional when not exposed;
- failure is a stable secret-safe category/message/remediation tuple.

## Capability inventory under test

The test fixture models a complete generation as:

```text
CapabilityGeneration {
  server_id,
  transport,
  tools: [{ name, description?, input_schema }]
}
```

Replacement assertions compare exact tool identities and schemas. Counts alone
are not authoritative because remove+add or schema-only changes can keep the
same count.
