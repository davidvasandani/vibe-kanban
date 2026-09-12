# Flexible collapsible panel stacks

The workspace right drawer (`packages/web-core/src/pages/workspaces/RightSidebar.tsx`)
is a bounded vertical flex stack whose expanded sections share all remaining
height. The shared expansion primitive is
`packages/ui/src/components/CollapsibleSectionHeader.tsx`.

## Expansion state must own flex participation

`CollapsibleSectionHeader` owns its live expanded state and persists it under
`vibe.ui.collapsible.*`. A feature container that supplies `defaultExpanded`
does not know the current state after a user toggles the section. It therefore
cannot reliably put expanded/collapsed flex classes on an outer wrapper.

For stacks that need this behavior, opt into `fillAvailableSpace`:

- expanded and collapsible: `flex-1 min-h-0`, so interactive open sections
  receive equal zero-basis shares and may shrink;
- collapsed or non-collapsible: `flex-none h-auto`, so the section consumes only
  its intrinsic height;
- omitted: the primitive retains its legacy `h-full min-h-0` behavior for
  unrelated callers.

Avoid duplicating expansion state in the feature container and avoid a CSS
`:has()` selector tied to the primitive's internal DOM. The state owner can
express the layout rule directly and testably.

## Complete the bounded flex chain

Equal flex shares work only if every ancestor from the height boundary to the
content scroller can shrink:

1. The drawer is full height and `min-h-0`.
2. Its section stack is a full-height `flex-col` with `min-h-0`.
3. Each expanded section root is `flex-1 min-h-0`.
4. The section body is `flex-1 min-h-0 overflow-auto` and remains the normal
   content overflow owner, leaving its header visible.

Do not add a fixed body minimum such as `min-h-[200px]`: it defeats shrinking
when several sections are expanded. Do not add viewport-derived section caps
when the drawer already defines the available height.

The outer drawer still uses `overflow-y-auto` as a fallback for unusually short
windows where the combined intrinsic height of all visible headers cannot fit.
Without that fallback, lower headers can be clipped and become unreachable.
Pair it with `overflow-x-hidden` so vertical overflow does not accidentally
create a horizontal scroll surface.

## Fixed chrome in a shared drawer component

The same `RightSidebar` composition is used by the desktop workspace drawer and
the mobile Git tab. Desktop-only fixed chrome therefore cannot be added
unconditionally inside `RightSidebar`: doing so duplicates mobile chrome and
changes a surface outside the desktop requirement.

Let the layout mount identify the desktop use explicitly, then render fixed
chrome as a first-child intrinsic row (`flex-none shrink-0`) before the mapped
collapsible sections. The row must not use `CollapsibleSectionHeader` or a
persisted expansion key when the product says it is always visible. This keeps
the row outside disclosure state while preserving all remaining-height sharing
for expanded sections.

Rendered-DOM coverage should assert both halves of the contract: the desktop
mount enables the row, and the row is first, intrinsic, and has no disclosure
button. Keep shared behavior such as formatting, timers, links, and accessible
labels in the existing presentational component rather than reimplementing it
at the drawer boundary.

## Mobile access uses the existing drawer destination

The mobile workspace layout routes the persisted `git` tab identifier to the
same `RightSidebar` composition. Keep that identifier stable even when the
user-facing affordance changes: it is both a saved UI preference and the layout
switch key.

The mobile control should describe the surface, not only one section inside it.
Use the mirrored right-sidebar icon, the visible label `Sidebar`, and the
accessible name `Right sidebar`. At phone widths visible labels are hidden, so
the accessible name is the durable meaning of the icon-only button. Native
button semantics plus `aria-pressed` accurately expose selection without
claiming the full ARIA tabs keyboard model.

Availability is route-owned. A workspace route ID makes the drawer destination
available even while its record is still loading; basing availability on fetched
workspace data resets a persisted drawer selection during ordinary refresh.
Omit the destination on the workspace-less landing and in create mode, and
recover an already-active `git` value to `workspaces` or `chat` respectively so
the layout cannot remain on hidden empty content.

## A collapsed header's data must already exist

A section whose collapsed state carries a summary (Constitution XXVI) must not
open a request or socket to produce it. `headerExtra` renders in the header row,
which survives collapse — so the *data source*, not the markup, is the design
problem.

Check what the layout already holds before adding anything. When the Pollers
section needed a live count ([[vk-pollers]]), the two obvious routes were both
wrong: copying `ServerMetricsHeader`'s private 30s `useQuery` violates XXVI
outright, and mounting `ExecutionProcessesProvider` in `WorkspacesLayout` opens a
**second** socket for a session the layout is already streaming
(`useExecutionProcesses(selectedSession?.id)`) while looking compliant. Passing
the already-fetched array down as a prop — as `repos` and `diffs` already are —
costs nothing and is trivially assertable: a test stubs `fetch` and asserts it is
never called.

## Verification

Rendered-DOM coverage should assert the shared primitive's opt-in expanded,
collapsed, intrinsic non-collapsible, and default root classes. Also inspect the
feature composition for a complete `min-h-0` chain and the absence of fixed
height caps. For a section with a collapsed summary, assert the summary survives
a header click and that the section issues no request of its own.

## Contributed by

- vk/b74a-right-drawer-exp
- vk/a12b-right-drawer-on
- VAS-377
- vk/869c-vk-background-po
