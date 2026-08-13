# Research: Close Stale Execution Follow-up Gaps

## Existing failure shapes

`ExecutionProcessesProvider.tsx` omits `setupscript` in its inline visible
predicate even though `useExecutionProcesses.ts` includes it. The provider value
is what `useWorkspaceExecution` consumes, so the pure helper test is not a test
of runtime composer truth.

The execution-process stream now subscribes before its snapshot, but sibling
snapshot streams still query first and ignore `BroadcastStreamRecvError`.
`MsgStore::history_plus_stream` reads history before subscribing and logs then
drops lag. Both patterns can create permanently incomplete state.

`useJsonPatchWsStream` allocates `dataRef.current = initialData()` before any
connection. Its failure check tests `!dataRef.current`, so it cannot fire. The
socket-open handler resets retries even if the connection never reaches Ready,
allowing repeated lag/resnapshot cycles at the shortest delay.

Codex emits `turn/completed` through its app-server client, and the local
container races executor exit signal against child exit before calling
`ExecutionProcess::update_completion`. Several worker event branches discard
completion-write errors. A final normalized message is separately extractable
from the message store, but presently does not arm recovery when terminal
signaling disappears.

## Decisions

### Subscribe first and fail authority on lag

Receiver acquisition before an awaited snapshot is the minimum lossless
handoff. Query-twice and client polling were rejected because each adds work
without making a missing patch explicit. Lag is an authority error, not a
diagnostic-only warning.

### Owner-specific liveness, shared reconciliation decision

Local OS/process-monitor evidence and remote worker job/lease/event evidence
cannot be collapsed into one probe without weakening the distributed authority
model. They can implement one decision contract: positive liveness permits
continued running; terminal evidence classifies exactly; absent evidence after
the bound becomes indeterminate after preservation.

### Forty-five seconds

The worker lease is normally 30 seconds. A 45-second testable bound covers one
lease plus event polling/scheduling margin. It is short enough to prevent the
confirmed indefinite spinner and does not turn final text into immediate exit.

### Indeterminate is the unknown-evidence terminal

The schema and UI already support `indeterminate`. It states exactly that the
coordinator cannot prove outcome. `completed` would overclaim; `failed` or
`interrupted` require evidence absent in this case.

### Ready resets health; open does not

A TCP/WebSocket handshake is transport availability, not stream authority.
Only the server's Ready boundary proves the connection supplied a complete
snapshot and should reset consecutive unhealthy retry pressure.

### Relay metadata is consumer-facing

The relay shim decodes a signed server close payload and currently calls the raw
browser socket's `close(code, reason)`. Reserved code 1011 cannot be originated
by browser script. The decoded envelope remains authoritative; close the raw
transport legally and synthesize one shim-facing CloseEvent with the decoded
metadata.

### No new dependencies

Tokio synchronization/time, existing broadcast wrappers, Vitest fake timers,
and current relay event helpers cover the needed behavior.

## Sibling stream audit disposition

- Execution processes, scratch, workspaces, and browser sessions all combine a
  DB snapshot with the shared event broadcast. They now call the same
  subscribe-before-snapshot helper and the same lag-to-error live adapter.
- `MsgStore::history_plus_stream` is history plus live state, not a DB snapshot,
  but has the same lossless boundary. `push` publishes while holding the history
  write lock; a subscriber takes the receiver while holding the read lock, so a
  message appears exactly once across history/live. Lag returns an I/O error.
- Raw and normalized live log WebSockets preserve that error through their
  filters. Stdout/stderr chunk streams now preserve it as well.
- The temporary historical-normalization store is exempt from resnapshot: it is
  a private, single-producer replay with the normal 100,000-message broadcast
  capacity and a Ready sentinel, not a concurrently changing authoritative DB
  snapshot. Its consumer owns and aborts the producer tasks if dropped.
