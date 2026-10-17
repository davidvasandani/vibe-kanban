# Mobile issue list rows

How the project **list view** (`IssueListView` → `IssueListSection` →
`IssueListRow` in `packages/ui/src/components/`) lays out rows on phones, and
how to check layout changes in it without a phone.

## The failure mode: a `shrink-0` cluster in a `justify-between` row

The desktop row is one flex line. The left cluster (handle, priority, id,
status dot, title) is `flex-1 min-w-0`. The right cluster (≤2 tags, ≤2
relationship badges, +N, assignee, age) is `shrink-0`, and its badges are
`whitespace-nowrap`. At 390px a row with a tag and two relationships makes
the right cluster wider than the row. The left cluster shrinks to 0, but its
own `shrink-0` children (handle, id) still paint, *underneath* the badges.
The age column (`w-5`) is pushed past the edge, so `52d` shows as `52c`.
`min-w-0` on a parent does not protect children that are `shrink-0`. Every
cluster that can hold variable content needs its own shrink or clip
boundary (constitution LIII).

## The mobile layout (`isMobile` prop)

`KanbanContainer` passes `useIsMobile()` down as an optional `isMobile` prop.
This is the same convention as `KanbanFilterBar`, `KanbanCardContent` and
`KanbanBoard`. The `ui` package does not decide layout itself.

- **Line 1:** handle/checkbox, priority, id, dot, title (`flex-1 min-w-0
  truncate`), age (`shrink-0`, no fixed width). The assignee also sits here
  when the issue has no tags or relationships, so a bare row stays one line.
- **Line 2** (only when tags or relationships exist): `pl-6` lines it up
  with the priority/id column (handle `w-4` + `gap-base`). The badges are in
  an inner `flex-1 min-w-0 overflow-hidden` box, and the assignee sits
  *outside* that box with `shrink-0`. Overflow therefore clips the last
  badge, never the avatar. With one `overflow-hidden` on the whole line,
  the avatar is the first thing clipped.
- The list scrollers (`IssueListView` root and the `KanbanContainer` list
  wrapper) carry `overflow-x-hidden`. `overflow-y-auto` alone makes them
  horizontal scrollers too (see [mobile-kanban-scrolling.md](mobile-kanban-scrolling.md),
  Gotcha 1).
- The desktop branch keeps its original markup. Both branches share the
  handle and metadata JSX through local helpers in `IssueListRow`.

Alternatives (one line with counters, stacked cards) are mocked up in
`specs/vk/2b39-fix-mobile-ui/mockups/`. They were rejected because counters
hide *which* issue blocks this one, and cards are about 3× taller and
duplicate the Board view.

## Rendering a real component at phone width without a phone

`packages/ui` has no test runner, and the task host (NixOS) has no system
browser. The Playwright Chromium in `~/.cache/ms-playwright` fails because
`libglib` is missing. What worked:

1. `nix build nixpkgs#chromium --no-link --print-out-paths` gives a working
   `bin/chromium` (`--headless=new --no-sandbox --screenshot=… --window-size=390,…`).
2. A temporary `packages/ui/src/__harness.tsx` renders the component with
   `renderToStaticMarkup` inside `<DragDropContext>` and writes HTML to
   `/tmp`. Bundle it with the root `node_modules/.bin/esbuild --bundle
   --platform=node --format=esm --jsx=automatic` plus a `createRequire`
   banner. Delete the harness afterwards.
3. CSS comes from the real config: run `packages/local-web/node_modules/.bin/tailwindcss -i
   ../web-core/src/app/styles/new/index.css -o /tmp/out.css` from
   `packages/local-web` (its content globs already include `../ui/src`).
   Render with `<html class="dark">` for the dark tokens.
4. `react-i18next` without an instance logs `NO_I18NEXT_INSTANCE` but
   returns the default strings, so `KanbanFilterBar` renders fine.

This checks layout only. It does not exercise touch gestures; use
`mobile-testing.md` for those.

## Contributed by

- vk/2b39-fix-mobile-ui
