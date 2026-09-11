# Implementation Plan: Streaming Conversation Viewport Stability

1. Preserve the task boundary
   - Work only in the Vibe Kanban repository.
   - Retain PR #268's edge-triggered plan reveal behavior.

2. Establish the SpecKit records
   - Refresh the constitution.
   - Create and clarify the task specification.
   - Record research, data model/state transitions, contracts, technical plan,
     tasks, and cross-artifact analysis under `specs/vk/9c15-still-shaking/`.

3. Reproduce and isolate the remaining jitter
   - Compare the recording frame by frame with PR #268's change.
   - Trace automatic earlier-history pagination, control rendering, anchor
     capture/correction timing, and streaming updates.
   - Identify the loading-state geometry change visible in the recording.

4. Add a failing regression
   - Add a rendered-DOM test for invariant earlier-history control geometry and
     relevant idle/loading/retry states.
   - Demonstrate that the focused test fails before the production fix.

5. Implement the narrow fix
   - Make the history-control region reserve invariant block space as its visual
     state changes.
   - Preserve existing semantic anchoring for actual page insertion.

6. Verify behavior
   - Run focused Vitest coverage first.
   - Install locked dependencies if required, then run relevant web-core type
     checks and lint, repository formatting, and `git diff --check`.
   - Recheck plan reveal, explicit navigation, interaction anchoring, and
     earlier-history behavior through existing tests.

7. Review and document
   - Run an independent Codex CLI review of the complete diff.
   - Address confirmed findings and repeat verification/review until no
     significant findings remain.
   - Update the project knowledge base with the stable mixed-virtualization
     invariant and task tag if confirmed.

8. Deliver
   - Commit the implementation and knowledge-base changes.
   - Push the task branch, open a pull request against the base branch, wait for
     required checks, address failures, and merge the pull request.
