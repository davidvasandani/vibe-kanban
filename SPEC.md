# SPEC — Org icons in the left drawer (AppBar rail)

**Task**: 3796 · vk-extended-left · Target: `vibe-kanban/` frontend (cloud shell)
**Full SpecKit artifacts**: `homelab/specs/vk/3796-vk-extended-left/`
(`spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, `tasks.md`)

## Problem
Switching Organizations requires opening the Org dropdown from the bottom-left
user popover — a slow multi-click detour when rapidly navigating Orgs+Projects.

## Solution
Extend the left drawer (vertical `AppBar` rail) so the user can **expand** an
Org section that shows every Org as an icon tile, styled exactly like the
existing Project tiles. Clicking an Org tile selects it (same effect as the
dropdown) and re-scopes the Project list below. The expanded/collapsed state is
persisted so the "extended drawer" choice sticks.

## Scope
- **In**: cloud app (`remote-web` → `RemoteAppShell`); shared `AppBar` /
  `AppBarOrgTile` (`packages/ui`); persisted UI state (`web-core`).
- **Out**: local-web parity; org CRUD/roles; org DnD reorder; backend/API/DB
  changes; adding a `color` field to the Org type.

## Functional requirements
- FR-1 One icon tile per Org the signed-in user belongs to.
- FR-2 Org tiles match Project-tile style (size, rounded, initials, hover,
  right-side tooltip with full name).
- FR-3 Clicking a tile selects that Org (equivalent to the dropdown).
- FR-4 Selecting re-scopes the Project list to that Org's projects.
- FR-5 Selected Org tile has a distinct active state (exactly one at a time).
- FR-6 Tiles grouped ("Orgs" label) in stable API order, visually separable
  from Projects.
- FR-7 Existing Org entry points (mobile `OrgSwitcher`, user popover) still work.
- FR-8 No breakage at 0 orgs (nothing rendered) or 1 org (single static tile).

## Design
- `AppBarOrgTile` gains `expanded?`/`onToggleExpanded?`. 0 orgs → null; 1 org →
  static tile; >1 org collapsed → active tile + caret-down toggle; expanded →
  `Orgs` label + vertical list of project-style tiles + caret-up toggle.
- `getOrgColor(id)` derives a stable HSL-triple (fixed S/L, hashed hue) for the
  active tile highlight — no data-model change.
- `useOrgRailStore` (zustand+persist, `org-rail-expanded`) holds `expanded`.
- `RemoteAppShell` passes store state into the `orgSlot` `<AppBarOrgTile>`.

## Acceptance criteria
- [ ] ≥2 orgs: expanding shows one project-styled tile per org above the
      Project list; clicking a non-selected tile switches org and the Project
      list updates without full reload.
- [ ] Exactly one tile shows the active state; hover shows the org name tooltip.
- [ ] 1 org → single tile, no toggle; 0 orgs → nothing, rail intact.
- [ ] Mobile Org switcher + user popover still function.
- [ ] Expanded/collapsed state persists across reload.
- [ ] `pnpm run check`, `pnpm run lint`, and the `AppBarOrgTile` Vitest pass.

## Files touched (`vibe-kanban/`)
- `packages/ui/src/components/AppBarOrgTile.tsx` (extend + `getOrgColor`)
- `packages/web-core/src/shared/stores/useOrgRailStore.ts` (new)
- `packages/remote-web/src/app/layout/RemoteAppShell.tsx` (wire-up)
- test alongside the component / `OrgSwitcher.test.tsx`
