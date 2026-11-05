# Feature Specification: Button prompts
Task: vk/bab2-button-prompts. The command template names a historical task path; this task uses its own artifact directory.
## Summary
Make common follow-up requests discoverable with three actions in an empty task composer.
## User Stories
As a task owner, I can continue work, ask what remains, or request status without typing.
## Functional Requirements
- FR-1: In an idle existing session with empty/whitespace text, replace Send with Continue, What’s left, Status in that order.
- FR-2: Submit respectively `Continue working on this task`, `What’s left on <linked issue simple_id>`, and `Status`.
- FR-3: Nonempty text, review-comment submissions, new sessions, running/queued states, approvals, questions and edits retain their existing actions.
- FR-4: Disable prompts while draft restoration is incomplete; disable What’s left when linked issue identity is unavailable.
- FR-5: Use existing send/recovery/cleanup behavior, and retain accessible keyboard-focusable buttons and a responsive footer.
## Out of Scope
Backend/API changes, automatic sends, deployment and other services.
## Acceptance Criteria
- Empty and whitespace editors show the three ordered actions and dispatch exact payloads.
- Typed text shows Send; draft-loading prompts and unresolved-issue action cannot dispatch.
- Running and special modes retain existing controls; existing resolver tests pass.
- Frontend type checks, lint and component tests pass; independent diff review has no significant findings.
## Open Questions
None. Existing Send replacement applies to idle existing sessions; other modes have distinct controls.

## Clarification review (/speckit.clarify)
No open questions remain. The issue display ID is simple_id, not its UUID.
Whitespace is empty; review comments continue to use Send. Prompt clicks do
not replace new-session creation, queue, stop, approval or answer controls.
