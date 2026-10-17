# Feature Specification: Recover Missing Codex Conversations

**Feature dir**: `specs/vk/af0d-no-conversation/`
**Status**: Clarified

## Summary

Allow a user to continue working in an existing Vibe Kanban workspace when the
Codex conversation referenced by that workspace is no longer available. The
current follow-up should start a replacement Codex conversation in the same
workspace instead of ending at `No conversation found`, while failures that do
not prove conversation absence remain visible.

## User Stories

- As a user returning to a workspace, I want my follow-up to run even when the
  underlying Codex conversation has disappeared so that I can keep working
  without recreating the Vibe Kanban workspace.
- As a user, I want the replacement conversation to become the continuation
  target so that later follow-ups work normally.
- As an operator, I want unrelated Codex failures to remain visible so that
  authentication, permission, configuration, and service faults are not hidden.

## Functional Requirements

- FR-1: A Codex follow-up with an existing external conversation reference must
  attempt to continue that conversation first.
- FR-2: When Codex specifically reports that the referenced conversation cannot
  be found, Vibe Kanban must create a new Codex conversation for the same
  workspace and run the pending follow-up in it.
- FR-3: The replacement conversation must use the same effective workspace,
  working directory, model, permissions, collaboration mode, configuration, and
  available integrations that a newly started Codex turn would use.
- FR-4: Vibe Kanban must record the replacement conversation identity through
  its normal session-identity flow so subsequent follow-ups continue the
  replacement conversation.
- FR-5: The pending user prompt must be submitted exactly once during recovery.
- FR-6: A successful continuation must remain unchanged and must not create an
  unnecessary replacement conversation.
- FR-7: Any failure that does not specifically establish that the conversation
  is missing must remain a failed turn with its diagnostic visible.
- FR-8: Recovery must not claim or imply that unavailable Codex-private context
  was reconstructed.
- FR-9: The behavior of other agent executors and unrelated Codex operations
  must remain unchanged.

## Out of Scope

- Reconstructing or synthesizing missing Codex-private transcript state.
- Changing the Vibe Kanban workspace's own visible conversation history.
- Retrying arbitrary Codex failures.
- Changes to any service other than Vibe Kanban.

## Acceptance Criteria

- [ ] A follow-up targeting a missing Codex conversation starts one replacement
      conversation in the same workspace and executes the follow-up.
- [ ] The next follow-up targets the replacement conversation identity.
- [ ] A follow-up targeting an available conversation continues it without
      starting a replacement.
- [ ] Authentication, permission, malformed-response, and generic internal
      errors do not trigger recovery.
- [ ] Focused automated tests cover positive and negative classification and
      the resume/start request sequence.

## Open Questions

None.
