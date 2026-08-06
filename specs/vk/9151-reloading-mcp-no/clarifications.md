# Clarifications: Reliable MCP Reload

## Resolved questions

### What ordering makes reload appear not to work?

The backend retains the canonical pending generation, but the chat container
does not load that status when a session is mounted or reselected. It instead
sets its local refresh result to `null`. If the user navigates, reloads the
page, or otherwise remounts while the generation is pending, the toolbar looks
idle again and stops polling.

Clicking the apparently idle control a second time does not restore tracking.
The coordinator correctly returns a transient `busy` projection for the
duplicate request while keeping its stored state `pending_next_turn`. The UI
stores `busy`, but its polling effect runs only for `pending_next_turn`, so it
never observes the later terminal result. This is the concrete lifecycle gap
to fix.

### Should the backend's duplicate-request behavior change?

No. Returning `busy` without advancing the generation is the existing
idempotency contract. The browser should reconcile from the canonical GET
status after a busy response and whenever it enters an existing session.

### What is the authoritative state?

The session-scoped backend coordinator status is authoritative. Component-local
state is only a view of it. A session change must clear stale visible data and
then fetch the selected session's current status with late-response protection.

### When should polling run?

Polling runs while the canonical selected-session status is
`pending_next_turn`. A duplicate POST that returns `busy` must trigger an
immediate canonical status read; if that read is pending, ordinary polling
continues. Terminal states stop polling and leave retry behavior consistent
with the returned result.

### Does this task change execution-side adoption semantics?

No evidence currently shows a violation in the execution handoff. The existing
backend deliberately distinguishes requests made before an execution starts
from those racing an already-resolving execution. This task preserves that
logic and adds regression coverage only where needed to protect the browser's
canonical-state reconciliation.
