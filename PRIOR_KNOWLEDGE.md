# Prior knowledge for "new workspace config must never be below the fold"

Distilled from `vibe-kanban/docs/knowledge-base/`. The knowledge base is not empty; three
pages bear directly on this task and one bears indirectly. Read-only pass — nothing was
changed here.

## 1. Nested flex scroll containment (`nested-flex-scroll-containment.md`, tags `vk/4f69-vk-create-issue`)

Most directly applicable page. Its rule is exactly the failure mode in the screenshots:

- Marking a child `overflow-y-auto` does **not** make it scroll. A flex item's automatic
  minimum block size stays content-sized, so it refuses to shrink into the space left by
  fixed siblings. When an ancestor clips overflow, the lower content is cut off rather
  than becoming reachable — content below the fold with no way to scroll to it.
- The complete contract is three-part, not one-part:
  `shell: flex flex-col h-full overflow-hidden` / `header: shrink-0` /
  `body: min-h-0 flex-1 overflow-y-auto`.
- The host must supply a definite height. **Do not duplicate host height with viewport
  units** (`vh`) or JavaScript measurement when the height chain is already definite.
  This is the strongest hit: the create composer's editor is currently capped with
  `max-h-[50vh]`, a viewport unit inside an already-definite container — precisely the
  anti-pattern this page warns against.
- Keep scroll ownership at exactly one level: the shell clips, the header does not
  shrink, only the body scrolls.
- Fix at the **shared presentational boundary** (`packages/ui`), not in mode-specific
  wrappers. Sticky submit actions, per-mode wrappers, and app-wide viewport changes were
  explicitly rejected there as blast-radius increases that create divergent scroll
  behaviour. Relevant here because `ChatBoxBase`/`CreateChatBox` are shared by the
  workspaces layout, the project right sidebar, and both local and remote frontends.
- Precedent outcome: on the last task of this shape, adding `min-h-0` to an existing
  `flex-1 overflow-y-auto` body was the whole fix, with no relocation of the lower
  controls.

## 2. Responsive flex toolbars (`responsive-flex-toolbars.md`, tags `vk/2163-fix-toolbar`)

The horizontal analogue, and the config row in question is a toolbar:

- Ownership of leftover inline space must be explicit:
  `row: flex` / `primary region: flex-1 min-w-0 overflow-x-auto` /
  `trailing region: shrink-0`.
- The zero minimum (`min-w-0`) is load-bearing: without it a flex item's automatic
  minimum content width pushes the trailing actions outside the viewport. The screenshots
  show the create action clipped at the right edge as well as the row clipped at the
  bottom, so both axes may be in play.
- Trailing actions stay `shrink-0`; safe-area padding belongs to the outer row and must
  not be reclaimed to make space.
- Layout ownership belongs to the shared `packages/ui` component so local and remote
  consumers stay consistent and route containers do not duplicate presentation policy.

## 3. Edge-triggered chat scroll intents (`edge-triggered-chat-scroll-intents.md`)

Indirect but worth honouring: asynchronous status presentations inside a scroll surface
must be geometrically invariant, and error feedback geometry should be reserved. The
create composer renders a conditional error alert and swaps the create button into a
"creating" spinner state; neither should be allowed to change the height budget in a way
that displaces the config row.

## 4. Shared testing convention (all three pages agree)

JSDOM does not calculate layout, scroll height, or pixel overflow. Every prior task of
this shape protected the browser-relevant contract with a **rendered-DOM structural
test** instead of geometry assertions:

- assert the shell carries the column/height/clipping constraints;
- assert the flexible body carries `min-h-0`, flexible growth, and auto overflow;
- assert the lower controls are descendants of the region that keeps them in view, not
  siblings outside it;
- use stable selectors, never brittle child indexes;
- pair with a pre-fix failure demonstration and manual/browser visual verification.

Each page also records the same verification ledger as its close-out: focused Vitest,
TypeScript checks, ESLint, repository formatting plus `git diff --check`, and an
independent Codex CLI review.

## Implications carried into spec and plan

1. Replace viewport-unit height caps in the create composer with container-relative
   flex sizing; the height chain from the workspaces layout (`flex-1 min-h-0
   overflow-hidden`) is already definite.
2. Apply the full three-part contract — the current layout centres content with
   `flex-1 items-center justify-center`, which has no clipping/shrink contract at all and
   can push content off both ends.
3. Make the heading and the "Run on" row shrinkable/non-authoritative; make the editor the
   single scrolling body; keep the config footer out of the scrolling region.
4. Prefer the shared `packages/ui` boundary, keeping `ChatBoxBase` behaviour intact for
   its session-chat consumer.
5. Cover it with a rendered-DOM structural test in the established lane, not a
   viewport-measurement test.
