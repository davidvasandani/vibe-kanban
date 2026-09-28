# SPEC: Fail fast when a workspace's assigned worker is offline

Task: `vk/ad2a-failed-to-start` (reported from VAS-747, Mycroft "Improve inference" interface)

## Problem

Starting a follow-up turn in a clustered workspace failed with:

```
Failed to start execution: worker transport failed: error sending request for url
(http://172.16.100.105:8086/v1/executions/b87cb4d6-a195-4f0b-b6fd-b35c77a16cff)
```

## Diagnosis (live cluster, 2026-09-28)

- The workspace is sticky-placed on **think5** (worker `001537ef…`,
  `http://172.16.100.105:8086`). The coordinator runs on think2.
- think5's `vibe-kanban-worker.service` is `active`, but its HTTP server has
  stopped accepting connections. The listen socket's accept queue is full
  (`Recv-Q 129` against a backlog of 128), and even `curl 127.0.0.1:8086/health`
  on think5 times out.
- Cause: a runtime `systemctl set-property` (applied 13:25 UTC, not declared in
  Nix) set `MemoryHigh=8G` / `MemoryMax=10G` on the worker unit. A poller in
  another workspace runs `go test` on a Microsoft Graph beta package, and one
  `go vet` child holds 5.3 GB RSS. Agent children share the worker's leaf
  cgroup, so the cgroup sits at `memory.high` (10.6M `high` events, about 8%
  `full` memory pressure). The kernel throttles every task in it, including the
  worker's own 13 MB control plane, which sits in D-state reclaim.
- The worker's heartbeats stopped. The coordinator marked think5 `offline` at
  17:07 UTC (last heartbeat 17:06:35). The coordinator's endpoint check has
  logged `unreachable endpoint http://172.16.100.105:8086` every minute since.
- Even so, at 18:52 `dispatch_execution` sent the new execution to think5. It
  checks only the workspace placement (`Ready` plus a `worker_node_id`) and
  never the assigned worker's registry state. It then waited out two 30s
  transport attempts (`retryable_dispatch_error` retries `Transport` once),
  about 60s in total, and returned an opaque transport error. The HTTP caller
  got a generic 500 "An internal error occurred".
- The transport error drops its cause. `reqwest::Error`'s `Display` shows only
  "error sending request for url (…)", so "operation timed out" and "connection
  refused" never reach the user.

## Goals

1. **Fail fast and say why.** When the workspace's assigned worker is not live
   (no registry row, `offline`, or lease missing or expired), refuse the
   dispatch before any worker-job record is created or any network call is
   made. The error names the worker's hostname and its last heartbeat, and
   tells the user to move the workspace to another execution server or retry
   once the worker recovers.
2. **Surface it as a typed, user-visible error.** The HTTP API returns
   `503 Service Unavailable`, error type `WorkerUnavailableError`, with the
   message above, instead of a generic 500. The execution's stderr log carries
   the same message.
3. **Keep the transport cause.** `WorkerClientError::Transport` renders the
   full `source()` chain, for example "…: operation timed out".

## Non-goals

- Automatically re-placing the workspace on another worker. Affinity is sticky
  by design ("never retry a dispatch on a different worker"). Moving a
  workspace goes through the existing affinity-migration flow.
- Changing retry counts or timeouts for a worker that is still `online` with a
  valid lease.
- Changing drain semantics. A `draining` worker with a valid lease keeps its
  current behaviour.
- Host remediation on think5 (the cgroup layout, the runtime `MemoryHigh`, the
  runaway `go vet`). That is operational and handled separately.

## Behaviour

| Assigned worker state | Before | After |
| --- | --- | --- |
| `online`, lease valid | dispatch | dispatch (unchanged) |
| `draining`, lease valid | dispatch | dispatch (unchanged) |
| `online`/`draining`, lease missing or expired | about 60s of timeouts, then an opaque 500 | immediate 503 `WorkerUnavailableError` |
| `offline` | about 60s of timeouts, then an opaque 500 | immediate 503 `WorkerUnavailableError` |
| no registry row | about 60s, then `EndpointNotFound` or an opaque 500 | immediate 503 `WorkerUnavailableError` |

Message shape:

> Execution server think5 is offline (last heartbeat 2026-09-28 17:06:35 UTC).
> Move this workspace to another execution server, or retry once think5
> recovers.

## Acceptance criteria

- Unit tests cover the liveness classification for every row in the table
  above.
- `ContainerError::WorkerUnavailable` maps to `ApiError::WorkerUnavailable`,
  which renders as a 503 with its message.
- A test shows that the `Transport` display includes the source chain.
- `cargo test` for the affected crates passes, and so do `pnpm run format` and
  clippy.
