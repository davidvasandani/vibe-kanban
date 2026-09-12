# Coordinator API Contract

Existing API response conventions remain in use.

## Worker administration

- `GET /api/worker-nodes`: nodes, health, draining, capacity, capabilities,
  mount diagnostic, heartbeat/lease age, and schedulability reason.
- `PATCH /api/worker-nodes/{id}`: administrator-only drain toggle and scheduling
  labels/weight overrides.
- `GET /api/worker-nodes/{id}/executions`: authoritative plus reconciled job
  ownership for diagnosis.

## Workspace placement

- Workspace create accepts optional `worker_node_id` and label constraints when
  cluster mode is enabled.
- Workspace responses include worker identity, placement state/time/reason, and
  a safe health summary.
- Invalid manual placement returns a specific conflict/bad-request response and
  does not provision a directory.

## Execution status

Execution responses add ownership and an explicit indeterminate/output-incomplete
representation. Existing completed/failed/killed meanings do not change.
Cancellation response distinguishes requested, acknowledged/escalating,
confirmed killed, already terminal, and worker unreachable.

## Streaming

Existing conversation WebSockets remain the browser contract. Coordinator
worker events are normalized/persisted before being emitted through those
channels, so browser clients do not connect directly to workers.

## Authorization

Worker administration uses existing local administrator authority. Workspace
and execution operations derive the target worker from persisted affinity;
clients cannot substitute a worker ID after placement.
