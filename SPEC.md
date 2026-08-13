# Mobile Toolbar Leading-Edge Follow-up — Technical Specification

## Objective

Prevent the first workspace tool from being partially clipped after the mobile
toolbar expansion shipped in `vk/2163-fix-toolbar`, while retaining equal use of
available space and fixed trailing actions.

## Requirements

1. Visible workspace tools fill the available mobile toolbar region.
2. The first tool is fully visible at the toolbar's leading edge when the tools
   fit in the available width.
3. When tools genuinely exceed the available width, only the tool group scrolls
   horizontally and all tools remain reachable.
4. Leading navigation affordances and trailing status/settings/account controls
   never shrink or become clipped by tool-group overflow.
5. Existing ordering, active styling, accessibility, project headers, desktop
   navbar behavior, and safe-area padding remain unchanged.
6. A rendered-component regression test protects the corrected flex/overflow
   ownership contract.

## Scope

Only the Vibe Kanban shared mobile navbar and its focused tests/documentation are
in scope. No other service or deployment configuration changes are permitted.
