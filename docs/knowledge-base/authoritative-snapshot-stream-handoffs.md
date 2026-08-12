# Authoritative snapshot and live-stream handoffs

Tags: `vk/3488-fix-stale-execut`

## Subscribe before taking the snapshot

A database snapshot followed by a broadcast subscription has a loss window. An
update committed after the query but before subscription exists is present in
neither source. For lifecycle state, one missed terminal update can leave a
client displaying `Running` forever.

Acquire the live receiver first, then await the authoritative snapshot. Emit
the snapshot before draining the receiver. Updates that happen during snapshot
capture are buffered and applied afterward. Duplicates are acceptable when the
collection is keyed and updates replace complete values; missing an update is
not.

## Lag invalidates stream authority

A bounded broadcast receiver can lag. Discarding its lag error and continuing
turns the patch stream into a silently incomplete source of truth. Surface lag
as a stream error, close the WebSocket with a non-clean retryable code, and make
the client reconnect for another full snapshot.

This rule applies to any snapshot-plus-patch stream: patches are an optimization
over resnapshot, never a substitute for it.

## Retain state during transport loss, replace it on reconnect

Do not clear a last known-good snapshot merely because the same endpoint is
reconnecting. That causes blank/flickering UI and does not establish a newer
state. Keep the snapshot rendered during the outage; every successful new
connection must then replace it with a complete authoritative snapshot before
continuing with patches.

Reset retained state only when the stream identity changes (for example, a new
session or endpoint).

## Derive activity from the closed status domain

Avoid a second local `isRunning` flag. Derive the action affordance from the
latest authoritative process records. For Vibe Kanban execution attempts, only
exact status `running` on an attempt-owned process shows Stop. `completed`,
`failed`, `killed`, `interrupted`, and `indeterminate` all clear the cancellable
running UI.

Keep a small truth-table test for this derivation and a rendered reconnect test
that starts stale-running and converges to a terminal snapshot.
