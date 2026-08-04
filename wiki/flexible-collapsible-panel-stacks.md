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

## Verification

Rendered-DOM coverage should assert the shared primitive's opt-in expanded,
collapsed, intrinsic non-collapsible, and default root classes. Also inspect the
feature composition for a complete `min-h-0` chain and the absence of fixed
height caps.

## Contributed by

- vk/b74a-right-drawer-exp
