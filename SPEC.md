# Technical Spec: Scrollable Create-Issue Settings

**Task ID:** `vk/4f69-vk-create-issue`<br>
**Service:** Vibe Kanban<br>
**Status:** Draft before implementation

## Problem

On constrained viewports, especially mobile, the create-issue panel's lower
settings and submit controls can extend below the visible panel. The panel is
intended to provide a vertically scrollable content region, but its flex sizing
does not reliably allow that region to shrink below its content's intrinsic
height. As a result, controls are cut off and the user cannot scroll to them.

## Scope

- Correct the create/edit issue panel layout so its content region becomes the
  scroll container within the available panel height.
- Preserve the fixed header, current control order, and existing create/edit
  behavior.
- Add regression coverage that asserts the panel shell and content region use
  the sizing and overflow contract required for scrolling.
- Limit code and documentation changes to the Vibe Kanban repository. No other
  service or deployment configuration is in scope.

## Technical Approach

The panel shell is a column flex container. Its scrolling child must be allowed
to shrink inside that container; in CSS flex layouts this requires a zero
minimum block size (`min-h-0`) on the relevant flex item (and, where necessary,
the shell). Retain `overflow-y-auto` on the content region and
`overflow-hidden` on the shell so scrolling occurs inside the issue panel rather
than leaking to an ancestor or the page.

Regression coverage will render `KanbanIssuePanel` and verify that:

1. the panel shell remains a height-constrained, overflow-clipping flex column;
2. the content region is a shrinkable flex child with vertical auto overflow;
3. create-only settings and the Create Issue action remain inside that region.

## Acceptance Criteria

1. On a short/mobile-height viewport, users can vertically scroll from the top
   of create-issue content through pipeline/settings controls to the Create
   Issue button.
2. The header stays outside the scrolling content and remains visible.
3. Edit-mode content continues to scroll and its section ordering is unchanged.
4. Existing keyboard, attachment, draft-workspace, pipeline, and submission
   behavior is unchanged.
5. Focused frontend tests, formatting, and relevant type/lint checks pass.

## Risks and Mitigations

- **Ancestor height contract differs across hosts:** keep the change local to
  the shared issue panel and test its explicit flex/overflow contract.
- **Accidental nested/page scrolling:** preserve the shell's clipped overflow
  and designate only the content child as vertically scrollable.
- **Visual regressions in edit mode:** use the same scrolling contract for both
  modes and retain existing DOM ordering.

## Verification

- Run the focused `KanbanIssuePanel` test suite.
- Run repository formatting.
- Run the relevant frontend typecheck/lint commands available in the workspace.
- Independently review the completed diff and address confirmed findings.
