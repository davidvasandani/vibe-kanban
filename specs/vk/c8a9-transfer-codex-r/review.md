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

Pending after fixes and focused verification.
