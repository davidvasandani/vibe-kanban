# Clarifications — 5276-debug-unrecogniz

No answers were supplied with the task, so each question is resolved with the
default that the constitution and existing code support. Each item records the reason.

## Q1. Clean up historical executions on read?
**Resolved: no. Leave recorded history as it is.**
Persisted execution logs are immutable evidence (constitution XVIII/XXXI). A
read-side filter would have to live inside each vendor parser and match
product-owned JSON by shape. That is the cross-layer coupling that the IX extension
forbids, and it would also hide lines that an agent emitted with the same shape. The
leak stops for every new execution, and older executions are rarely reopened.

## Q2. Show cancellation phases anywhere?
**Resolved: drop them from the chat and trace them on the coordinator.**
The phases are progress bookkeeping. The terminal `Killed`, `Interrupted`, or `Indeterminate`
event that follows already carries the outcome, and the UI already renders it.
An indeterminate kill already produces its own stderr line. A coordinator
`tracing::debug!` keeps the phase available to operators without adding chat noise
(constitution IX: "bookkeeping that carries no user-facing fact is not rendered at all").

## Q3 (raised during clarify). How should worker errors be worded?
**Resolved:** `Worker error: <reason>` and `Worker output stream error: <reason>`,
pushed as stderr, with the reason text verbatim. Unrecognised structured
metadata is pushed as `Unrecognized worker event: <json>` on stderr. This keeps the
text available as evidence (XI) and never routes it through a vendor parser.
