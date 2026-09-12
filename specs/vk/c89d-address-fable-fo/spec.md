# Feature specification: reliable workspace creation

Task: vk/40fb-workspace-creati
Feature directory: specs/vk/c89d-address-fable-fo/ (exact path named by repository SpecKit commands)
Status: specified

## Summary
Investigate and correct intermittent workspace creation failures so valid requests complete reliably and failures remain useful to diagnose without risking existing work or duplicate agents.

## User stories
- As a user, I can create a workspace from valid repositories and start its initial agent without intermittent internal failures.
- As an operator, I can correlate a creation failure to its workspace and cause.
- As a user, failed creation never destroys existing work or starts duplicate agents.

## Functional requirements
- FR-1: Correct the cause demonstrated by creation logs and reproducible code behavior.
- FR-2: Preserve single-consumer lifecycle transitions and request-independent execution.
- FR-3: Preserve repository/branch identity and shared-worktree ownership; retries must not replay committed startup side effects.
- FR-4: Keep permanent failures visible and safely diagnosable by workspace identity; never expose credentials.
- FR-5: Verify the corrected failure scenario and successful creation behavior with focused regression coverage.

## Out of scope
Other services, speculative retries, unrelated UI redesign, dependency upgrades, and destructive recovery of failed workspaces.

## Acceptance criteria
- [x] A documented causal chain connects operational or reproducible evidence to the correction.
- [x] Regression tests reproduce the defect and pass with the correction.
- [x] Successful creation, permanent failure, and no-duplicate behavior remain correct.
- [x] Required formatting/checks and independent review have recorded outcomes.
- [ ] Knowledge is recorded and the task PR is merged.

## Open questions
Resolved from coordinator logs: Sep 12 failures name repository administration lock contention; Sep 11 failures report a rejected Provisioning-to-Ready transition. Investigate both mechanisms, fixing those supported by reproducible evidence. Unsupported CURSOR_AGENT placement also appears in older logs and must remain a visible permanent failure. No user product decisions remain open.
