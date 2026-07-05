# Mobile kanban board scrolling & snapping

How the mobile kanban board (`packages/web-core/src/features/kanban/ui/KanbanContainer.tsx`,
board primitives in `packages/ui/src/components/KanbanBoard.tsx`) handles
touch scrolling, and the CSS gotchas that have repeatedly bitten it.

## Architecture: nested single-axis scrollers

On mobile (`useIsMobile()`), the board is deliberately split so each touch
gesture is owned by exactly one scroller per axis:

- **Outer board scroller** (`KanbanContainer`): `overflow-x-auto snap-x
  snap-mandatory overflow-y-hidden` — horizontal only. Owns column paging;
  each column is `auto-cols-[100vw]` with `snap-start` (KanbanBoard grid).
- **Per-column card list** (`KanbanCards` mobile classes): `overflow-y-auto
  overflow-x-hidden overscroll-y-contain min-h-0` — vertical only.
- Column headers live inside each column, *outside* the card list, so they
  move with the column during horizontal scrolls.

The browser routes a pan gesture to the nearest ancestor that is
user-scrollable on the gesture's axis. Keeping each container scrollable on
exactly one axis is what makes horizontal swipes page columns and vertical
swipes scroll cards — regardless of where the touch starts.

## Gotcha 1: `overflow-y: auto` promotes `overflow-x` to `auto`

Per the CSS overflow spec, `visible` on one axis combined with a scrolling
value on the other computes to `auto`. Writing only `overflow-y-auto` on an
element silently makes it a *horizontal* scroll container too. If anything
inside overflows horizontally — even by 1px — that element starts competing
with an ancestor horizontal scroller for pan gestures, and on iOS the inner
scroller wins and rubber-bands.

**Rule: any mobile vertical scroller nested inside the board must also carry
`overflow-x-hidden`.**

## Gotcha 2: the `-mx-[1px]` card border trick creates real overflow

`KanbanCard` renders with `-mt-[1px] -mx-[1px]` negative margins so adjacent
card borders and the column `divide-x` borders collapse into single 1px
lines. Side effect: every card list's `scrollWidth` exceeds its
`clientWidth` by ~1–2px. Harmless while the list clips (`overflow-x-hidden`),
but combined with Gotcha 1 it turned every card list into a genuinely
scrollable horizontal container.

## Gotcha 3: `touch-action: pan-y` is not the fix for stolen swipes

It looks tempting to stop a card list from panning horizontally with
`touch-action: pan-y`, but that prevents horizontal gestures starting on
cards from reaching the *outer* board at all — users could then never swipe
columns from the card area. Constrain what an element can scroll
(`overflow-*`), not what gestures may begin on it (`touch-action`).

## History

- `986ce39b` (#46): full-width `100vw` columns + `snap-x snap-mandatory`
  board scroller.
- `d65b91e1` (#47): split into the nested single-axis scrollers above,
  because one container scrolling both axes let vertical swipes drift
  columns sideways.
- vk/de6e-improve-column-s: #47 regressed card-origin horizontal swipes
  (Gotchas 1+2 — cards panned/rubber-banded, columns/headers stayed, board
  rested between snap points). Fixed by adding `overflow-x-hidden` to the
  mobile card-list classes.

## Verifying changes here

There is no touch engine in the task environment; `mobile-testing.md` (repo
root) documents the phone-over-Tailscale flow. Minimum manual checks after
touching this area: card-origin horizontal swipe pages columns (header+cards
together, snaps on release); vertical swipe scrolls only that column;
long-press drag handle still drags cards; desktop unchanged; repeat in Slim
view (same code path).

## Contributed by

- vk/de6e-improve-column-s
