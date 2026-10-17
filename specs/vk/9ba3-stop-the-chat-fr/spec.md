# Feature Specification: Stable Streaming Chat Viewport

**Feature dir**: `specs/vk/9ba3-stop-the-chat-fr/`
**Status**: Clarified

> The checked-in `/speckit.specify` prompt names
> `specs/vk/c89d-address-fable-fo/spec.md`, an existing unrelated feature.
> Following the task-owned `.speckit-owner` marker and current branch identity,
> this run writes to the current feature directory instead of overwriting prior
> work.

## Summary

Keep the workspace chat readable while an agent streams output. The conversation
must follow the live tail when the user is already there, preserve the current
reading position when the user scrolls away, and never bounce repeatedly between
vertical positions without user input.

## User Stories

- As a user watching an active agent, I want new chat output to appear without
  the existing content jumping so that I can read progress as it arrives.
- As a user reading earlier output during an active turn, I want my chosen
  position preserved so that background updates do not interrupt me.
- As a user navigating or expanding conversation content, I want those explicit
  actions to retain their existing predictable scroll behavior.

## Functional Requirements

- FR-1: While the conversation is following the live tail, appended entries and
  growth of the active entry keep the viewport at the live tail without
  oscillating to an earlier position.
- FR-2: After the user scrolls upward, streaming updates do not return the
  viewport to the live tail until the user explicitly returns or invokes a
  bottom-navigation action.
- FR-3: A conversation row retains one stable layout ownership strategy during
  an active streaming lifecycle; transient presentation entries must not move a
  row repeatedly between layout regions.
- FR-3a: Plan reveal is an edge-triggered navigation event. Once a plan-exit
  entry has been revealed, later snapshots whose latest entry is still that same
  plan-exit entry are ordinary streaming updates and must not reveal it again.
- FR-4: Explicit navigation to a turn, patch, or bottom position continues to
  place the requested content predictably.
- FR-5: Expanding or collapsing interactive chat content preserves the activated
  control's visible position.
- FR-6: Loading earlier history preserves the first visible semantic row and
  remains retryable on failure.
- FR-7: Returning to the live tail retains the existing bounded-history release
  behavior.
- FR-8: The behavior is shared by every frontend that consumes the workspace
  conversation feature.

## Out of Scope

- Chat visual redesign, message copy changes, or composer changes.
- Backend streaming protocol or persisted conversation schema changes.
- Homelab deployment or configuration changes.
- Replacing bounded history or the existing single-scroller architecture.

## Acceptance Criteria

- [ ] A deterministic regression test demonstrates that the first observation
      of a plan-exit entry emits plan reveal, repeated snapshots with that same
      entry retain their incoming update type, and a distinct later plan reveals
      once.
- [ ] At the live tail, a sequence of appended and resized streaming rows yields
      continuous bottom-follow behavior with no earlier-position jump.
- [ ] Away from the live tail, the same update sequence preserves the reader's
      anchor and does not engage bottom-follow behavior.
- [ ] Existing history-prepend, turn navigation, entry navigation, and
      interaction-anchor tests or focused verification continue to pass.
- [ ] Frontend type checks, lint, formatting, and relevant unit tests pass.
- [ ] Independent Codex review reports no significant findings.

## Open Questions

None. The clarify stage established that the recorded oscillation is caused by
repeated plan-reveal intent, not by active-tail boundary ownership. The loading
row and authoritative process status therefore keep their current roles.
