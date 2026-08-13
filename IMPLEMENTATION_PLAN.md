# Mobile Toolbar Leading-Edge Follow-up — Implementation Plan

1. Refresh task-specific SpecKit artifacts and validate them against the project
   constitution.
2. Add a failing rendered-DOM test proving fixed leading controls sit outside
   the horizontally scrollable tool region.
3. Refactor the mobile workspace navbar so the outer leading region grows but
   does not scroll, while only the inner tool group owns overflow.
4. Preserve equal-width tool expansion, minimum tap widths, fixed trailing
   controls, safe-area padding, and all existing behavior.
5. Run focused tests, shared UI/web-core checks, lint, formatting, and diff
   validation.
6. Run independent Codex review until no significant findings remain.
7. Update and commit reusable knowledge, then open and merge a pull request.
