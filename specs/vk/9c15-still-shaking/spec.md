# Feature Specification: Stable Streaming Conversation Viewport

**Feature dir**: `specs/vk/9c15-still-shaking/`
**Status**: Clarified

## Summary

Keep the workspace conversation readable while an agent streams output by
preventing repeated involuntary vertical movement. The viewport must follow new
content when the reader is at the live tail and preserve the reader's chosen
position after they scroll away, without either policy fighting conversation
layout changes.

## User Stories

- As a user watching a running agent, I want prior lines to remain visually
  stable so that I can read output as it arrives.
- As a user who scrolls upward during a run, I want my position retained so that
  new output does not interrupt review of earlier content.
- As a user navigating the conversation, I want plan reveal, previous-message
  navigation, and history loading to keep working after streaming is stabilized.

## Functional Requirements

- FR-1: A conversation at its live tail must follow appended and growing output
  without alternating between vertical offsets.
- FR-2: A conversation away from its live tail must preserve the reader-selected
  viewport during ordinary streaming updates.
- FR-3: The system must assign one scroll policy at a time: follow tail, preserve
  anchor, or execute explicit navigation.
- FR-4: The earlier-history control must keep stable block geometry when it
  changes among idle, loading, retry, and completed-page states.
- FR-5: Automatic consecutive page loads must not expose an intermediate layout
  shift before semantic anchor correction runs.
- FR-6: A new conversation scope must start from clean viewport lifecycle state.
- FR-7: One-shot plan reveal introduced by PR #268 must remain one-shot and must
  not become a recurring streaming command.
- FR-8: Initial load, previous-message navigation, entry navigation, interactive
  row expansion, and earlier-history loading must retain their established
  behavior.

## Out of Scope

- Conversation visual redesign.
- Backend streaming or persistence changes.
- Deployment changes, homelab changes, or changes to any other service.

## Acceptance Criteria

- [ ] A deterministic regression reproduces the implicated active-turn state
      sequence and fails before the fix.
- [ ] The earlier-history control reserves the same vertical space while idle
      and loading.
- [ ] Bottom-locked live growth results in a stable, monotonic viewport policy.
- [ ] Scrolling upward releases tail following until the user explicitly
      returns to the bottom.
- [ ] Loading, retry, exhausted-history, and scope-reset transitions are covered
      in proportion to their layout impact.
- [ ] Existing plan-reveal and scroll-intent tests remain green.
- [ ] Focused tests, frontend type checks/lint, formatting, and diff checks pass.
- [ ] Independent Codex review reports no significant findings.
- [ ] A pull request against the base branch is merged.

## Open Questions

None. Resolutions are recorded in `clarifications.md`.
