# Clarifications: Show Sent Chat Messages Without Refreshing

## 1. How long is response-derived execution state retained?

**Decision:** Retain it until the active session's live execution projection
reports the same process ID or the session changes.

A successful follow-up response is positive, server-confirmed creation evidence.
Expiring it after an arbitrary client timer would make the user turn disappear
again during a prolonged WebSocket outage. When live delivery reports the same
ID, the live value becomes authoritative and the response-backed bridge can be
discarded. A session change discards all bridge state to preserve identity
isolation.

## 2. Is this a pre-acceptance optimistic message?

**Decision:** No. Reconciliation happens only after the follow-up API succeeds
and uses the exact execution process returned by the server. Failed requests do
not create UI state.

## 3. Which source wins when both arrive?

**Decision:** Values from the live execution stream supersede the response value
for a matching process ID. Both delivery orderings remain idempotent.

## Remaining questions

None.
