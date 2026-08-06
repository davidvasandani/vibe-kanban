# Independent review: Codex rollout lineage transfer

## Pass 1

Command: `codex review --base vk/9a64-vk-workspace-aff`

The review reported three P1 findings:

1. **Confirmed:** the stage route inherited Axum's 2 MiB default body limit,
   below the protocol's encoded 32 MiB artifact maximum. Fixed by applying the
   worker's existing 72 MiB signed-body limit to JSON extraction and adding a
   regression test above 2 MiB.
2. **Rejected after inspection:** the review said a transfer-failure response
   left the operation claimed. The response is returned from the inner `result`
   future; the enclosing `match result` calls `finish_operation` for every
   `Ok` response, including `SessionTransferFailed`, so the claim is durably
   completed before the HTTP response.
3. **Confirmed:** `safe_existing_path` rejected a final symlink but could follow
   a parent symlink that remained within the sessions root. Fixed by validating
   every relative component with `symlink_metadata` before opening and adding
   an in-root parent-symlink regression test.

## Pass 2

The second review reported one P1 finding:

1. **Confirmed:** after successful target verification, failure of the durable
   `source_stop_started` write could exit before cancellation while leaving the
   source quiesced. Fixed by explicitly resuming the source on that boundary;
   the subsequent cancellation-error path already performs the same
   compensation.

## Pass 3

The third review reported two findings:

1. **Confirmed (P1):** unconditional rollout-store creation made every worker's
   startup depend on available writable Codex storage, including non-Codex-only
   workers. Fixed by making the store optional at startup and returning a
   specific transfer error only when a transfer route needs unavailable
   storage.
2. **Confirmed (P2):** a crash-left deterministic partial caused
   `create_new` to reject a retry. Fixed by removing only the exact regular,
   non-symlink partial for the same operation/thread before recreating it;
   unexpected file types remain a safety error. The idempotency test now seeds
   this crash artifact.

While applying these fixes, the compensation path was also tightened so an
owned quiescence can be resumed from nonterminal `Cancelling` or
`Indeterminate` worker states, and an unconfirmed stop result explicitly tries
that compensation rather than waiting for the lease watchdog.

## Pass 4

The fourth independent review reported no significant findings.

## Acceptance follow-up

After the clean pass, a manual acceptance cross-check found that the explicit
age-based cleanup policy was documented but not yet wired. Transfer receipts
and a bounded six-hour worker sweep were added: 24-hour partial cleanup always
runs, while 30-day verified cleanup runs only when the worker has no active
executions. Native/pre-existing Codex rollouts are never receipted merely for
matching content, so cleanup considers only artifacts installed by Vibe
Kanban. A fifth independent review covers this follow-up.

## Pass 5

The fifth review found two P1 issues. Migration could select a thread from an
older execution when the active turn had not persisted an ID, and cleanup
could remove a rollout still referenced by an idle resumable session. Migration
now reads only the active execution's turn. Workers clean abandoned partials
but do not delete verified rollouts without coordinator-supplied reference
proof.

## Passes 6–9

Subsequent reviews found and verified fixes for these recovery cases:

- stale recovery re-verifies already verified target lineage before dispatch;
- incomplete stale transfers resume idempotent staging rather than attempting
  final verification prematurely;
- a failed post-install checksum removes only the destination still sharing
  the temporary file's inode;
- the selected target worker is persisted and reused across automatic-affinity
  retries; and
- distinct simultaneous `parent_thread_id` and `forked_from_id` metadata is
  rejected instead of silently omitting ancestry.

The repeated claim that `SessionTransferFailed` leaves an operation claimed
was rejected again: the inner success response always reaches the outer
`finish_operation` call.

## Final result

The final Vibe Kanban review reported: “No discrete correctness issue was
identified that would clearly warrant a code change.” A separate review of
homelab commit `1feb2d85` found the `CODEX_HOME` pin, private directory, and
evaluation assertions consistent with the worker service account and reported
no breaking behavior.
