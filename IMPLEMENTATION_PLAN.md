# Implementation plan: reliable Claude background-Bash denial

**Task:** `VAS-540` / `vk/5cd1-debug-this-vk-ba`

1. Refresh the task branch from the current base.
   - Inspect upstream changes since the workspace branch was created.
   - Merge the current `origin/main` before implementation so testing targets
     the deployed Claude executor and pin.
   - Preserve the task artifacts created by this pipeline.

2. Establish the exact failing lifecycle.
   - Trace ownership of Claude child stdin/stdout, the detached protocol reader,
     terminal-result grace, cancellation, EOF, and child wait/finalization.
   - Inspect the pinned Claude Code artifact's structured-input request broker
     and hook ordering around `DENY_BACKGROUND_BASH_CALLBACK_ID`.
   - Reproduce the failure with a deterministic duplex-stream test or the
     smallest realistic harness that captures control request, response, result,
     and stream-close ordering.

3. Correct protocol ownership.
   - Model outstanding control requests explicitly if they can outlive the
     reader iteration or terminal-result signal.
   - Do not close/drop the input stream until terminal handling has allowed all
     accepted required responses to flush, while retaining a bounded fallback
     for a genuinely wedged CLI.
   - Propagate or record response write failures instead of logging and
     converting them into apparent success.
   - Keep cancellation responsive and retain zero-turn-result protection.

4. Preserve hook semantics.
   - Keep explicit `run_in_background: true` denied in auto, supervised, and
     plan modes.
   - Keep foreground/malformed Bash routed through existing permission policy.
   - Retain the actionable `spawn_poller` denial message and all tool-name
     guardrails.

5. Add regression coverage.
   - Add protocol transport tests for background denial at the terminal
     boundary and any identified race ordering.
   - Cover clean terminal shutdown, cancellation, and response write failure if
     touched by the fix.
   - Retain/extend unit tests for hook routing and denial payload shape.

6. Verify and document.
   - Run focused `executors` tests first, then formatting and proportionate
     workspace checks after installing dependencies if frontend tooling is
     involved.
   - Record the root cause and reusable lifecycle invariant in the existing
     project knowledge page and refresh its index.
   - Run independent Codex diff review, address confirmed findings, and rerun
     affected checks until no significant findings remain.

7. Ship.
   - Commit task source/artifacts and the knowledge-base update.
   - Push the task branch, open a pull request against the base branch, wait for
     required checks, address failures, and merge only when the review and CI
     gates are clear.
