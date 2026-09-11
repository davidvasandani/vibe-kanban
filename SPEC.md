# Stop Workspace Chat Viewport Shaking

## Problem

While a workspace agent is actively streaming a turn, the conversation viewport
can jump repeatedly between substantially different vertical positions even
though the user is not scrolling. In the supplied 13.38-second recording, the
same recent-turn content alternates between the upper and lower portions of the
chat roughly every half-second. The message composer and surrounding workspace
panels stay fixed, isolating the defect to conversation-list layout/scroll
correction.

This makes live output difficult to read and can make controls move away while
the user is trying to interact with them.

## Scope

- Diagnose the feedback loop between live conversation updates, mixed
  virtualized/unvirtualized row layout, row measurement, and bottom locking.
- Stabilize the workspace conversation viewport during active streaming.
- Preserve intentional behavior: following new output when bottom-locked,
  retaining a reader's position after scrolling upward, turn navigation,
  expanding/collapsing entries, and lazy-loading earlier history.
- Add focused regression coverage for the state transition or scroll decision
  responsible for the oscillation.
- Change only the Vibe Kanban source repository. Homelab deployment and other
  services are out of scope.

## Functional Requirements

1. A bottom-locked conversation follows appended or growing live output without
   oscillating between old and new scroll positions.
2. A user who scrolls upward during streaming remains anchored to the content
   they chose; live updates must not pull them back to the bottom.
3. Moving rows across the virtualized-tail boundary must not introduce a
   repeating height/scroll correction loop.
4. Existing programmatic navigation and interaction-anchor corrections must
   continue to work.
5. Initial load, settled conversations, and earlier-history pagination must
   retain their current behavior.

## Acceptance Criteria

- A deterministic regression test reproduces the implicated streaming/layout
  transition and fails before the fix.
- During continuous live updates, the viewport has a single stable scroll
  policy: it stays at the bottom when locked or stays on the reader's content
  when unlocked; it does not alternate between those states without user input.
- Focused frontend tests, type checking, linting, and formatting pass.
- Independent Codex review reports no significant findings.
- The change is documented in the project knowledge base if it yields reusable
  guidance, then delivered through a merged pull request.

## Non-Goals

- Redesigning chat presentation or message rendering.
- Changing backend streaming protocols or persisted conversation data.
- Modifying homelab deployment configuration.

## Evidence and Initial Technical Direction

The recording shows large vertical jumps confined to the chat scroller while a
turn is running. The current list combines a TanStack Virtual head with a normal
DOM tail and recalculates the boundary based on active-streaming state. It also
performs bottom-lock corrections in a layout effect on row-count and total-size
changes. Investigation should determine which boundary or measurement change
makes the scroll height alternate, then make that transition monotonic or keep
the reader anchored without weakening the existing bottom-follow behavior.
