# Research: Authoritative Execution Status Reconciliation

## Root cause

`stream_execution_processes_for_session_raw` first awaits
`ExecutionProcess::find_by_session_id` and only afterward calls
`msg_store.get_receiver()`. If completion is committed and broadcast between
those operations, the new stream has a stale `running` snapshot and cannot
receive the already-sent terminal patch. The client faithfully retains and
renders that stale snapshot, including after the reconnect that hit the race.

## Decision: subscribe before snapshot

Create the broadcast receiver before querying the snapshot. Updates occurring
during the query remain buffered and are chained after snapshot plus `Ready`.
The latest keyed update wins, so both initial connections and reconnects
converge without a new API, cursor, or polling loop.

Query-twice was rejected because a second query still has a boundary before
subscription unless paired with the same ordering, and it adds database work to
every connection. Client polling was rejected because it masks rather than
repairs the authoritative stream contract.

## Frontend behavior

`useJsonPatchWsStream` deliberately retains initialized data during reconnect,
then applies the server's replacement snapshot. That provides continuity and is
correct once the server snapshot handoff is lossless. Clearing data immediately
on disconnect would hide valid active state and produce flicker without proving
terminal status.

`useExecutionProcesses` defines running with exact equality to `running`.
Consequently completed, failed, killed, interrupted, and indeterminate already
clear the composer once received. No parallel local running flag needs to be
introduced.

## Shutdown behavior

Coordinator-local orphan cleanup tries safe process adoption, preserves WIP,
and marks unrecoverable non-persistent rows interrupted. Worker-owned rows are
left for evidence-backed reconciliation, which can classify uncertainty as
indeterminate. Focused verification is required, but the observed stale UI does
not justify weakening these lifecycle rules.

No new dependency is required.
