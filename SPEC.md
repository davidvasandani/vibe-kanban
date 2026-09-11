# Stabilize the Streaming Conversation Viewport

## Problem

PR #268 made `ExitPlanMode` navigation edge-triggered, removing the large
plan-reveal/bottom-follow oscillation. The supplied 7.44-second, 3160×2034
recording shows a second defect remains: while the agent is streaming and the
viewport is following the tail, already-rendered conversation content repeatedly
moves vertically by a smaller but visible amount. The composer and surrounding
workspace chrome remain stationary, isolating the motion to conversation-list
layout and scroll correction.

Frame-by-frame review shows this is continuous tail-follow jitter rather than a
new user scroll or a repeat of the large plan reveal jump fixed in PR #268.

## Scope

- Diagnose the interaction among automatic earlier-history pagination, its
  loading presentation, semantic anchoring, and streaming updates.
- Keep the conversation viewport stable while new content streams.
- Preserve initial positioning, explicit navigation, plan reveal, expansion
  anchoring, and earlier-history anchoring.
- Add deterministic frontend regression coverage for the implicated layout or
  scroll-policy transition.
- Change only the Vibe Kanban source repository. Homelab deployment and all
  other services are out of scope.

## Requirements

1. When bottom-locked, appended or growing live output follows the bottom with
   one coherent correction policy; already-rendered content must not bounce
   because the list alternates between competing measurements or boundaries.
2. When the reader scrolls upward, streaming updates must preserve the chosen
   viewport and must not re-acquire bottom lock without an explicit user action.
3. Entering and leaving the earlier-history loading state must not change the
   vertical position of already-visible conversation rows.
4. Consecutive automatically loaded history pages must keep one stable anchor
   throughout their loading and commit phases.
5. PR #268's one-shot plan reveal semantics must remain intact.
6. Existing previous-message navigation, entry navigation, interaction-anchor
   correction, and earlier-history pagination must retain their behavior.

## Acceptance Criteria

- A deterministic regression test fails on the pre-fix behavior and covers the
  state sequence visible in the recording.
- During continuous live updates, sampled viewport positions are monotonic when
  content grows at the bottom and do not alternate between two offsets.
- The earlier-history control has stable layout geometry across idle, loading,
  retry, and success transitions.
- The focused frontend suite, type checks, lint, formatting, and diff checks
  pass.
- Independent Codex review reports no significant findings.
- Reusable knowledge is recorded in the project knowledge base and the change
  is delivered through a merged pull request.

## Non-Goals

- Redesigning conversation presentation or changing entry rendering.
- Changing backend streaming, persistence, or execution lifecycle semantics.
- Modifying `homelab/modules/vibe-kanban-rebuild.nix` or any other service.

## Initial Technical Hypothesis

The remaining jitter is downstream of PR #268. In the recording, the centered
“Loading earlier messages” presentation repeatedly appears as the conversation
moves downward, then disappears as the content returns upward. In
`ConversationListContainer`, the idle load-earlier button and loading skeleton
occupy different normal-flow heights. The history anchor is captured before
`loadEarlier()`, but its correction loop begins only after the awaited request
finishes, so the intermediate loading render is visibly uncorrected. Automatic
pagination can repeat the down/up cycle for consecutive pages. The narrow fix
should keep the control region's block geometry invariant across loading states,
leaving semantic anchoring to correct only actual history insertion.
