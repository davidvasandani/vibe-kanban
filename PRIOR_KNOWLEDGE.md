# Prior Knowledge — recalled for `vk/3796-vk-extended-left`

Searched the project knowledge base (`wiki/`) for pages relevant to this task
(left drawer / `AppBar` rail, org navigation, project/org icon tiles, sidebar).

## Relevant findings

**None directly relevant.** The knowledge base has five pages
(`wiki/INDEX.md`); none covers the `AppBar` rail, the org switcher, or the
project/org icon-tile UI this task touches:

- `wiki/electric-sync-fallback.md` — client Electric sync + REST fallback and a
  navbar *banner*. Only tangential (mentions the navbar), nothing about the
  left rail or org tiles.
- `wiki/kanban-items-state-and-activity-grouping.md` — kanban items ↔ DnD
  index/sort_order. Unrelated (though the project list in the rail uses
  `@hello-pangea/dnd`, this task adds no DnD).
- `wiki/mobile-kanban-scrolling.md` — mobile board scroll/snap. Unrelated.
- `wiki/self-hosted-deployment.md` — deploy pipeline. Unrelated.
- `wiki/project-context-map.md` — monorepo scope mapping. Unrelated.

So on the topic of **the left-drawer AppBar rail and org navigation tiles**,
this is effectively a first task — no prior page to build on.

## Carry-forward facts established during this task's investigation
(useful to the spec/plan stages; candidates for the knowledge base afterwards)

- The left drawer is the vertical **`AppBar` rail**:
  `packages/ui/src/components/AppBar.tsx`. It renders an `{orgSlot}` ReactNode at
  the very top, then labeled sections; the `project-list` case is the styling
  template for icon tiles (colored initials, 40×40 `rounded-lg`, right-side
  `Tooltip`, active state via inline `hsl(color)`).
- Orgs are a **cloud** concept surfaced only in `remote-web`
  (`RemoteAppShell.tsx`), which today passes a single-tile/dropdown
  `AppBarOrgTile` as the `orgSlot`. Org data:
  `useUserOrganizations()` + `useOrganizationStore` (both `web-core`).
- `OrganizationWithRole` has **no `color`** field (unlike `Project.color`), so
  org-tile coloring must be client-derived.
