# Authoritative snapshot and live-stream handoffs

Tags: `vk/3488-fix-stale-execut`, `c89d-address-fable-fo`,
`vk/8a08-frontend-not-ref`, `vk/113f-sidebar-randomly`

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

## Final output is a reconciliation trigger, not exit evidence

A normalized final assistant response can arrive before executor/process
finalization. It must never be promoted directly to `completed`, but it is
strong enough to start a bounded watchdog. At expiry, check owner-specific
positive liveness: a local child that still returns `try_wait() == None`, or a
remote active job with a current lease, defers destructive recovery. Once that
evidence disappears, transition to the most truthful non-running status,
usually `indeterminate`, and preserve the diagnostic.

Later tool, interaction, or start events disarm the final-output timer. Log
history may be byte-capped, so reconciliation cannot use retained history
length as its change detector.

Worker terminal events remain unacknowledged until their authoritative process
row is persisted. Retry the captured evidence in place, then acknowledge both
the coordinator record and worker replay cursor before releasing the tracker.

## Readiness and reconnect pressure are separate

An open WebSocket is transport evidence, not snapshot authority. Reset retry
pressure only after `Ready`; otherwise repeated open-before-Ready failures can
create an aggressive resnapshot loop. Track authoritative readiness separately
from allocated placeholder data so initial failure becomes visible while a
previous valid snapshot remains rendered during reconnect.

When relay framing carries a server close such as 1011, emit its code and reason
to consumers but close the browser-owned underlying socket without attempting
to originate a reserved close code. Preserve `wasClean` for normal code 1000 so
clean completion does not reconnect.

## Bridge acknowledged creations into the streamed projection

A mutation response can be durable creation evidence even when the collection's
live notification is delayed or missed by the current connection. Discarding
the created record and waiting only for a patch creates a user-visible gap: the
mutation succeeds, local input clears, and the new entity does not appear until
a later full snapshot.

Reconcile the complete server-returned entity into the same ID-keyed projection
consumed by the UI. This is response-backed reconciliation, not a fabricated
pre-acceptance optimistic record. Apply live snapshot/patch values last so they
supersede the bridge value, then retire the bridge once live delivery contains
that ID. Keeping a separate unkeyed UI record forces fragile matching and risks
duplicates.

Scope the bridge to the collection identity. Validate late responses against the
current scope at callback time, filter the merged projection by that scope, and
clear bridge state when scope changes. Tests must exercise response-first,
stream-first, live supersession/removal, and a response settling after a scope
switch.

## A failed refresh is not an empty snapshot

Workspace sidebar names and pins come from the workspace stream, while PR,
diff, approval, poller and unseen-activity metadata come from a separate bulk
summary query in `packages/web-core/src/shared/hooks/useWorkspaces.ts`. Losing
all enrichment while retaining names can therefore indicate a summary-cache
failure rather than workspace deletion.

Reject failed summary requests. Returning an empty map after an HTTP, API or
transport failure tells React Query that an authoritative empty snapshot arrived
and replaces the last successful metadata. Rejection preserves same-key cached
data and permits normal retry/poll recovery. A successful empty array or explicit
null/false/zero fields must still replace the previous snapshot.

Do not use `keepPreviousData` to retain data through same-key refresh failures;
React Query already retains it. That placeholder also carries data across query
keys and can show another host's metadata when workspace IDs overlap. Keep host
and archive status in the query identity. Test the real QueryClient with failed
refreshes, automatic polling recovery, explicit clears, initial failure, and
late responses after a host switch.

Contributed by: `vk/113f-sidebar-randomly`.
