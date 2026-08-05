# Worker Protocol Contract

Protocol messages are versioned, authenticated, size-bounded, and carry
`coordinator_id`, `worker_node_id`, timestamp, nonce, and correlation ID.

## Registration and heartbeat

- `POST /v1/register`: stable identity, versions, capabilities, resources,
  labels, mount validation evidence. Returns accepted protocol range,
  heartbeat interval, lease duration, and current probe challenge.
- `POST /v1/heartbeat`: current resources/mount state and active job summaries.
  Returns coordinator acknowledgement and optional drain/config state.
- `GET /v1/jobs`: active and retained jobs with execution ID, request digest,
  state, last sequence, and terminal evidence.

## Execution

- `POST /v1/executions/{execution_id}`: immutable dispatch containing workspace
  ID/path, session ID, action/profile, working directory, environment/secrets,
  reason, timeout, persistence, and request digest.
  - same ID/digest: return existing accepted job;
  - same ID/different digest: conflict;
  - wrong worker/path/assignment: forbidden.
- `GET /v1/executions/{execution_id}/events?after=N`: WebSocket or streaming
  response of events strictly increasing from `N+1`.
- `POST /v1/executions/{execution_id}/ack`: highest contiguous persisted
  sequence.
- `POST /v1/executions/{execution_id}/cancel`: idempotent cancellation request
  and grace/escalation policy; response never claims killed before evidence.

## Events

Every event contains execution ID, sequence, worker timestamp, and one payload:
accepted, starting, stdout bytes, stderr bytes, structured executor message,
approval/question request, preview metadata, completed, failed, killed,
indeterminate, or replay-gap.

The worker retains a configured bounded journal. If `after` predates retained
history it returns the earliest retained cursor and an explicit replay-gap.

## Interaction

- `POST /v1/executions/{execution_id}/interactions/{interaction_id}` correlates
  approval/question response, deadline, and expected request type.
- Worker acknowledgement is returned and also journaled.

## Terminal and preview

- `POST /v1/terminals` validates workspace affinity/path and returns a terminal
  session ID.
- `GET /v1/terminals/{id}/stream` carries bounded input/output/resize/close
  frames.
- Preview tunnels authorize `(workspace, job, port, generation)` and connect
  only to worker loopback.

## Safety limits

Reject oversized environment, event, terminal frame, label, capability, or
diagnostic payloads. Redact configured secret values. Apply bounded queues and
backpressure; never buffer an unbounded disconnected stream in memory.
