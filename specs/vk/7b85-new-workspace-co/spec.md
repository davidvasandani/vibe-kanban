# Feature Specification: New workspace config never falls below the fold

**Feature dir**: `specs/vk/7b85-new-workspace-co/`
**Status**: Clarified

## Summary
On narrow viewports the Create Workspace screen can push its configuration controls — the
model/preset selector, the attachment and repository-summary controls, the linked-issue
badge and the Create action — out of sight below the bottom edge of the panel that hosts
it. The reported case is a phone opening the "Create Workspace" panel for an issue with a
long prompt pre-filled: the prompt fills the screen, the config row is cut off, and
because the host panel clips its overflow the user cannot scroll to reach it, so the
workspace cannot be configured or created at all. This feature makes those controls
permanently visible at every supported viewport size by having the prompt area yield space
instead of the controls.

## User Stories
- As someone creating a workspace on a phone, I want the agent, model, repository and
  Create controls to stay on screen no matter how long my prompt is, so that I can finish
  creating the workspace without fighting the layout.
- As someone creating a workspace from an issue with a long pre-filled pipeline prompt, I
  want the prompt to scroll within its own area, so that reading or editing the prompt
  never hides the controls that commit it.
- As someone using the Create Workspace panel in the project sidebar on a desktop, I want
  the screen to look and behave exactly as it does today when the prompt is short, so that
  the fix costs me nothing.
- As someone choosing repositories in the first step of create mode, I want that step to
  stay usable within the same panel height, so that the fix is not limited to the prompt
  step.

## Functional Requirements
- FR-1: In the create-workspace screen, the configuration control row and the create
  action MUST be fully visible within the height the host gives the screen, for every
  supported viewport size and for any prompt length.
- FR-2: When the content needs more room than the host provides, the prompt area MUST be
  the part that gives way: it shrinks and scrolls its own content.
- FR-3: Exactly one element owns vertical overflow in the create-workspace screen. The
  screen MUST NOT rely on scrolling the surrounding page, and MUST NOT leave content
  clipped in a region that offers no way to scroll to it.
- FR-4: The step heading, the placement ("Run on") row, the linked-issue warning, the chat
  header and the config row MUST NOT be compressed or displaced to make room for the
  prompt. Space is surrendered in one order: the prompt area first and completely, and
  only then anything else. The config row and the create action are never surrendered.
- FR-10: The prompt area absorbs the whole deficit, shrinking and scrolling down to a small
  floor of roughly two lines — below that it would be as unusable as a hidden config row.
  The screen therefore requires only its fixed cost (heading, placement row, chat header
  and config row) plus that floor, which is below every supported viewport height; that is
  what makes FR-1 achievable without hiding anything.
- FR-12: Below the supported range — including when the duplicate-workspace advisory is
  present on a short window — the create action MUST remain reachable by scrolling rather
  than be clipped away. The screen owns its overflow; it never hands unreachable content to
  its host.
- FR-11: The step heading keeps its present size and is not part of the visibility
  guarantee in FR-1. In practice it stays visible because the prompt area absorbs the whole
  deficit; the guarantee itself covers only the config row and the create action.
- FR-5: Transient state — a validation or creation error, and the create button's in-flight
  label — MUST NOT change whether the config row is visible.
- FR-6: The repository-selection step of create mode MUST remain fully usable under the
  same height constraint, and its Continue action — the control that commits that step —
  MUST stay inside the host's height for the same reason the create action does. The
  repository list yields and scrolls; the controls row does not.
- FR-7: When the content fits, the screen MUST keep its current appearance, including the
  vertically centred composition.
- FR-8: Chat surfaces other than create mode MUST keep their present sizing behaviour; the
  new height behaviour applies only where it is asked for.
- FR-9: The behaviour MUST hold in every host that renders the create-workspace screen —
  the mobile chat tab, the desktop workspaces panel, and the project right sidebar's
  Create Workspace panel — and in both the local and the remote frontend.

## Out of Scope
- Redesigning the create-workspace screen, its controls, or the set of configuration
  options offered.
- Changing which controls appear in the config row, or their order or grouping.
- The session chat composer used inside an existing workspace.
- Any deployment, hosting, or non–Vibe Kanban service change.
- Horizontal overflow behaviour of the config row, which already has its own contract.

## Acceptance Criteria
- [ ] With a long prompt at a narrow mobile viewport, the config row and the create action
      are fully within the panel; nothing is cut off at the bottom edge.
- [ ] With a long prompt, the prompt area scrolls internally and the surrounding screen
      does not scroll.
- [ ] With a short prompt, the screen is indistinguishable from today's, including vertical
      centring.
- [ ] Showing a validation error, and putting the create button into its in-flight state,
      each leave the config row fully visible.
- [ ] The repository-selection step renders and remains operable at the same narrow
      viewport.
- [ ] Automated coverage asserts the structural contract — the prompt area carries a zero
      minimum and owns vertical overflow, the config row does not shrink, and the config
      row is not inside the scrolling region — and asserts that the default, non-create
      chat surface carries none of it. The prompt area is required to yield space, not to
      grow into it.
- [ ] Repository type checks, lint, and formatting pass; an independent Codex review
      reports no significant findings.

## Open Questions
All three questions raised at `specify` time were resolved at `clarify` time. None of
them changed the shape of the work, so none warranted blocking; the resolutions are
recorded here and folded into the requirements above.

- **Resolved — should the heading shrink, wrap differently, or hide at short viewport
  heights?** No. The heading keeps its current size and does not compress (FR-11). Once the
  prompt area can shrink to zero and scroll, the screen's fixed cost is small enough that
  the heading never has to yield at a supported viewport size. Hiding or resizing it would
  be an unrequested design change, and gating it on a viewport-height media query would
  reintroduce exactly the viewport-unit height duplication that constitution XXXVII forbids.
- **Resolved — is there a minimum prompt-area height?** No floor on the scrolling region
  (FR-10). The prompt keeps a one-line minimum for its own content, but the region that
  contains it may shrink to zero so the deficit never reaches the config row. The prompt
  scrolls rather than being truncated, so no content is lost at any size.
- **Resolved — does "below the fold" cover the heading too?** No. The reported defect and
  the guarantee are about the configuration controls and the create action (FR-11). The
  heading is preserved in practice but is not what the requirement protects.

## Residual notes
- A viewport shorter than the screen's fixed cost plus the prompt floor is outside the
  supported sizes. Acceptance does not require the config row to be *visible* there, only
  reachable (FR-12). The duplicate-workspace advisory adds materially to the fixed cost, so
  it is the realistic way to reach that range on a short desktop window.
- Verification of the pixel-level outcome needs a real browser. Automated coverage asserts
  the structural contract only, because the test environment computes no layout.
