# Durable session recovery
VK owns recovery on the server. Persist the original session, workspace, operation identity, attempt count, last confirmed step and execution ownership before dispatch. Classify failures from typed errors, exit status and bounded diagnostics. A successful process printing npm upgrade information requires no action. A failed process printing that notice follows the ordinary failure classifier. Dispatch only predefined workspace-scoped operations.

For transient errors during a read-only workspace check, repeat the check once and record the original failure, retry and verification in session history. Continue the pending step through an atomic claim only after verification. Unsupported and exhausted failures retain diagnostics and a concrete manual next action.

At startup reconcile only active work owned by the interrupted service instance. Check executor identity and durable completion evidence before takeover. Reattach survivors, resume confirmed safe checkpoints, and request review for uncertain side effects. Cancellation or pause wins over recovery. Replayed events never dispatch work.

Acceptance tests should interrupt a dedicated VK instance during execution and during reconciliation, with no browser connected. Count side effects, inspect persisted history, replay events and check inactive controls.
