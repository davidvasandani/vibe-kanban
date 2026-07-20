# Implementation Plan: Dispatch Queue Before Early Finalization

1. Confirm the no-change cleanup-skip branch is the only path gated by
   `already_finalized` before normal queue consumption.
2. Extract the repeated scratch-delete/start behavior into one local helper.
3. In the no-change branch, claim and start a queued message before choosing
   manual finalization; fall back to finalization if absent or start fails.
4. Keep the normal finalization branch on the same helper to avoid behavior
   drift.
5. Add focused decision regression tests, run local-deployment tests/checks and
   formatting, then complete independent review and knowledge capture.
