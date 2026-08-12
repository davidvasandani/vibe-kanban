# Implementation Plan: Authoritative Execution Status Reconciliation

Task: `vk/3488-fix-stale-execut`

1. Reproduce the UI failure by retaining a running snapshot across an
   unexpected close, missing the terminal patch, and reconnecting.
2. Assert that a full terminal replacement snapshot on reconnect changes the
   rendered state from running to interrupted without clearing the UI during
   transport downtime.
3. Subscribe the session execution stream to broadcasts before awaiting its
   database snapshot so updates during snapshot capture are buffered.
4. Chain snapshot, Ready, then buffered/live updates, preserving the existing
   keyed JSON Patch contract.
5. Turn broadcast lag into an explicit stream error and close the WebSocket with
   retryable code 1011 so the browser must reconnect and resnapshot.
6. Extract and test the exact running-attempt derivation: active coding-agent
   work remains cancellable; completed, failed, killed, interrupted, and
   indeterminate statuses clear Stop.
7. Verify existing restart/shutdown behavior with focused local deployment
   coverage and retain evidence-based worker reconciliation.
8. Run formatting, focused frontend/Rust tests, TypeScript checks, server
   compilation, and diff validation.
9. Run independent Codex review until no significant findings remain.
10. Record reusable snapshot/live handoff knowledge, commit it, open the task
    pull request against the recorded base branch, and merge after checks pass.
