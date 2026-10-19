# Feature specification: VK Browser scrolling

Applied `/speckit.specify`. Status: specified.

## Summary and user stories
As a controlling browser user, I can swipe or use a mouse wheel/trackpad to read remote content beyond the visible viewport, without moving the VK page or accidentally clicking content.

## Functional requirements
- FR1: Vertical and horizontal wheel/trackpad input scrolls remote content at the pointer location.
- FR2: A one-finger swipe scrolls content in the expected direction and proportion at any rendered frame size.
- FR3: Touch movement beyond an 8 CSS-pixel threshold is a swipe; taps retain keyboard/click behavior. Swipes never trigger a click or keyboard focus.
- FR4: Only the controlling connection can scroll. Interrupted, multi-touch, disconnected or old-session gestures cannot become clicks or resume after a control change.
- FR5: Input outside the rendered frame does not scroll remote content. The controlled surface owns scrolling; read-only viewers preserve native host gestures.

## Acceptance criteria
Rendered component tests verify both scroll axes, letterboxing/scaling, pixel/line/page wheel units, touch tap and swipe behavior, cancellation, additional touches and control/session changes. Existing clipboard tests continue to pass.

## Out of scope
Pinch zoom, inertia simulation, remote drag-and-drop, new transport endpoints and hosting changes.

## Open questions
None. “Add scrolling” means both touch and wheel input for the existing remote browser.

## Clarification review
Applied `/speckit.clarify`: one-finger swipes move content with the finger; wheel direction follows device deltas. Support both axes. Eight CSS pixels distinguish taps from swipes. Multi-touch cancels remote gestures; no simulated momentum or pinch zoom. No unresolved questions.
