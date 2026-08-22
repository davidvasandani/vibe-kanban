# Implementation Plan: Clear stale Stop after turn completion

**Task:** `vk/7655-turn-ends-aren-t`

This plan starts from `SPEC.md` and `../PRIOR_KNOWLEDGE.md`. SpecKit stages may
refine file-level details after the root cause is reproduced.

1. Establish the current lifecycle contract and reproduce the failure.
   - Trace the selected session from `SessionChatBoxContainer` through
     `ExecutionProcessesProvider`, `useExecutionProcesses`, and the JSON Patch
     stream.
   - Trace local and cluster-worker turn completion through terminal status
     persistence and execution-process broadcasts.
   - Compare the reproduction with the earlier snapshot/subscription race and
     bounded final-output reconciliation fixes to identify the remaining or
     regressed boundary.

2. Add a focused failing regression at the owning boundary.
   - Model the observed sequence: a process is `running`, final assistant
     output is visible, the process reaches a terminal outcome, and the client
     must converge to that terminal record.
   - Include the relevant reconnect, event-replay, executor, or finalization
     scenario based on the root cause rather than testing transcript text as
     completion evidence.

3. Implement the smallest authoritative lifecycle fix.
   - If terminal persistence is missing, repair the executor/container or
     coordinator-worker finalization path with bounded, evidence-backed status
     reconciliation.
   - If terminal persistence is correct but delivery is stale, repair the
     snapshot/patch stream or reconnect handoff so a terminal snapshot replaces
     cached running state.
   - Keep `hasRunningAttempt` as the single UI activity derivation and preserve
     all terminal-status, dropped-process, session-switch, queue, approval, and
     work-preservation semantics.

4. Verify proportionally to the changed layers.
   - Run the new regression and adjacent frontend/Rust lifecycle tests.
   - Run `pnpm install --frozen-lockfile` if this fresh worktree is not already
     prepared, then formatting, type/backend checks, and applicable lint/tests.
   - If deployment wiring changes, restrict it to
     `homelab/modules/vibe-kanban-rebuild.nix` and run Nix parse/evaluation
     checks from the homelab repository.

5. Complete the required delivery pipeline.
   - Run an independent Codex diff review and address all confirmed significant
     findings until clean.
   - Update the existing authoritative-stream or agent-lifecycle knowledge page
     with reusable findings, tag it `vk/7655-turn-ends-aren-t`, refresh the
     knowledge index, and commit the knowledge base.
   - Open a pull request against the base branch, wait for required checks as
     needed, and merge it.
