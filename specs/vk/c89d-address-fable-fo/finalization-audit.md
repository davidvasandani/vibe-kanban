# Execution Finalization Audit

## Local Codex boundary

`CodexClient::on_notification` recognizes `turn/completed`; the JSON-RPC reader
then sends `ExecutorExitResult::Success`. `spawn_exit_monitor` races that signal
against positive OS child exit. Previously, closure of the signal channel was
classified as success, and failure of the single `update_completion` write was
only logged. Both could leave an authoritative running row while normalized
assistant output was already visible.

The repaired boundary:

- channel closure is failure evidence, never assumed success;
- non-empty normalized assistant output followed by 45 seconds of log silence
  arms reconciliation, but does not prove exit;
- normal executor/OS terminal evidence wins immediately;
- at the quiet bound the still-owned local process group is reaped and the row
  becomes `indeterminate` through bounded completion-write retries;
- finalization continues through the existing repository/queued-action path.

The silence window resets whenever new normalized/raw history arrives, so a
streaming response or later tool activity is not mistaken for process exit.

## Cluster worker boundary

Ordered worker events are authoritative. Completed/failed/killed/interrupted
events retain their exact mapping; replay gaps already become indeterminate.
Previously, several terminal writes discarded errors, and a final structured
assistant patch with no later terminal event could poll forever.

The repaired worker tracker records a 45-second quiet deadline after final
assistant output, resetting it on later worker events. If no terminal event
arrives, it sends bounded cancellation to the exact assigned worker, marks the
worker job indeterminate, retries the execution completion write, emits a
diagnostic, finalizes, and closes the store. A failed completion write keeps the
tracker alive to retry rather than publishing Finished over a running row.

## Preservation and restart

The existing startup orphan path remains responsible for WIP capture when a
server/process owner disappears across restart. This change does not alter the
shutdown-side missing-handle early return that keeps the row discoverable by
that sweep. In-process bounded reconciliation acts only while the container or
worker tracker still owns the execution and can reap/cancel it before
classification.

## Status truth table

- successful terminal evidence → `completed`;
- failed exit/evidence → `failed`;
- explicit cancellation/interruption evidence → existing `killed` or
  `interrupted` path;
- final output + bounded silence + no terminal outcome → `indeterminate`;
- final assistant text alone never → `completed`.
