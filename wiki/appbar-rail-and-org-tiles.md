# AppBar rail: sections, icon tiles, and the org switcher

The left "drawer" is the vertical **AppBar rail** (`packages/ui/src/components/AppBar.tsx`),
a presentational component. Understanding its slots and the tile recipe makes
adding new rail affordances (like the org switcher) low-risk.

## Anatomy
- `AppBar` renders, top-to-bottom: an `{orgSlot}` ReactNode (very top,
  `AppBar.tsx:523`), then labeled **sections** (`Local`, `Remote`, `Projects`,
  `Export`) built as a data array, then a bottom utility cluster
  (command bar, settings, notification bell, `{userPopover}`, version).
- The rail scrolls: root has `overflow-y-auto`, so a tall stack of tiles
  (many orgs + many projects) degrades gracefully — no need to cap counts.
- Sections are data (`AppBarSection[]`) rendered by `renderSectionItem`; the
  `project-list` case (`AppBar.tsx:~460`) is the canonical **icon-tile recipe**.

## The icon-tile recipe (reuse this for any rail tile)
- 40×40 (`w-10 h-10`), `rounded-lg`, `text-sm font-medium`, focus ring —
  captured in `appBarItemBaseClassName`. Content is 2-letter initials via a
  `getInitials`-style helper (first letters of first two words, else first 2
  chars, uppercased).
- **Active** state uses an *inline* `style` (not a class): text
  `hsl(<color>)` + background `hsl(<color> / 0.2)`. **Inactive** state is
  `bg-primary text-normal hover:opacity-80`. Projects get `<color>` from
  `Project.color` (an HSL-triple string).
- Each tile is wrapped in a right-side `<Tooltip side="right">` with the full
  name for the collapsed rail.

## Org switcher in the rail (`AppBarOrgTile.tsx`)
Orgs are a **cloud** concept; the rail org tile is wired only in
`remote-web` (`RemoteAppShell.tsx`) via the `orgSlot` prop. To switch orgs
without the buried user-popover dropdown, `AppBarOrgTile` renders an
expand/collapse section of project-styled org tiles:
- 0 orgs → `null`; 1 org → a single **non-interactive** tile; >1 org →
  collapsed = active tile + caret toggle, expanded = an `Orgs` label + one
  tile per org + collapse toggle.
- Expanded/collapsed state is persisted in `useOrgRailStore`
  (`packages/web-core/src/shared/stores/useOrgRailStore.ts`, zustand+`persist`,
  key `org-rail-expanded`). Note `useExpandableStore` is **not** persisted, so a
  dedicated persisted store is used rather than overloading it.
- **Org color has no data-model field.** Unlike `Project.color`,
  `OrganizationWithRole` has no `color`. Derive a stable hue on the client:
  hash the org id → `getOrgColor(id)` returning `"<h> 65% 55%"` (fixed S/L so
  only hue varies and stays in the tuned dark-rail range). Don't add a DB/type
  field just for tile coloring.

## Gotchas (both caught in Codex review of this task)
1. **Optional controlled state must have a working fallback.** Exposing
   `expanded?`/`onToggleExpanded?` as *optional* on an exported component means
   a caller can pass neither. If the toggle handler is a no-op when omitted, the
   whole switcher becomes dead (can't expand, can't reach `onSelect`). Fix:
   treat "handler present" as controlled; otherwise fall back to internal
   `useState`. Call the hook **before** any early `return` (single-org / no-org
   branches) to respect rules-of-hooks.
2. **Don't render a `<button>` with a no-op onClick.** The single-org tile has
   nothing to do, so render a non-interactive `<div>` (keep `aria-label`/
   `aria-current`), not a focusable button that announces a dead action to
   keyboard/screen-reader users. Pattern: a shared `OrgTile` that emits a
   `<button>` only when it has an `onClick`, else a `<div>`.

## Where things live / data sources
- Org list: `useUserOrganizations()` (web-core, React Query → `GET
  /v1/organizations`). Selected org: `useOrganizationStore` (web-core, persisted;
  syncs to `useUiPreferencesStore` for server persistence).
- Selecting an org re-scopes projects automatically in `RemoteAppShell` via
  `activeOrganizationId` → the `["remote-app-shell","projects",orgId]` query; no
  extra wiring needed when adding a new org-selection entry point.
- Other org entry points that must keep working: the mobile `OrgSwitcher`
  (drawer header) and the `AppBarUserPopover` org list.
- Component tests for `@vibe/ui` live in `packages/remote-web/src/app/layout/`
  (e.g. `OrgSwitcher.test.tsx`, `AppBarOrgTile.test.tsx`) and must be run from
  the `remote-web` package (its own vitest config + testing-library deps).

## Contributed by
- vk/3796-vk-extended-left
