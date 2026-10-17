# Research Notes

## Existing send response is discarded

`sessionsApi.followUp` returns `Promise<ExecutionProcess>`, and the backend
returns the process produced by `start_execution`. `useSessionSend` currently
awaits this call without retaining the returned value, then tells the composer
to clear. This is the narrowest reliable source for closing the UI gap.

## Existing conversation discovery

`useConversationHistory` discovers running processes from
`ExecutionProcessesContext`, creates an empty keyed process state immediately,
and connects to the process's normalized-log stream with bounded retry. Feeding
the server-returned process into the existing provider therefore makes the user
prompt available through the process action and retains the normal log/status
path.

## Existing stream authority

`useExecutionProcesses` consumes
`/execution-processes/stream/session/ws`. Initial snapshots and patches are
keyed by execution ID. The generic WebSocket hook retains last known-good data
during same-endpoint reconnect and resets on endpoint/session changes.

## Alternatives rejected

- Add a synthetic optimistic user entry in the chat component: rejected because
  it duplicates normalized conversation identity and needs fragile later
  matching/removal.
- Refetch or reconnect the full process stream after every send: rejected
  because it is heavier, can flicker, and still introduces timing dependencies.
- Change the backend stream first: rejected because the response already carries
  sufficient durable creation evidence and request/stream reconciliation is
  required even with a lossless stream.
- Expire response state on a timer: rejected because an arbitrary timeout would
  make the accepted message disappear during longer transport outages.

## Dependencies

No new package, API, database migration, generated type, or service dependency
is required.
