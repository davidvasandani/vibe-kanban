# Contract: Session Execution-Process Stream

For every initial WebSocket connection and reconnect:

1. Establish a live event subscription before capturing the database snapshot.
2. Emit `JsonPatch([{ op: "replace", path: "/execution_processes", value:
   <map keyed by process id> }])`.
3. Emit `Ready`.
4. Emit every relevant update buffered since subscription, then live updates.

An update committed during snapshot capture must appear either in the snapshot
or in the buffered updates (duplicates are allowed); it must never appear in
neither. Reducing the ordered messages must produce the latest authoritative
status.

Unexpected disconnect causes reconnect/resnapshot. A clean finished stream does
not reconnect. The client may retain the prior snapshot while reconnecting, but
must replace it when the next connection's snapshot arrives.
