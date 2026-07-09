# Implementation Plan — Org icons in the left drawer (AppBar rail)

Frontend-only change to the cloud app. See `SPEC.md` for rationale and full
SpecKit artifacts under `homelab/specs/vk/3796-vk-extended-left/`.

## Step 1 — Persisted expander store (new file)
`packages/web-core/src/shared/stores/useOrgRailStore.ts`
- zustand + `persist`, mirroring `useOrganizationStore`.
- State: `expanded: boolean` (default `false`), `toggleExpanded()`,
  `setExpanded(v)`. `persist` name `org-rail-expanded`,
  `partialize` → `{ expanded }`.

## Step 2 — `getOrgColor(id)` helper
In `packages/ui/src/components/AppBarOrgTile.tsx` (co-located):
- Deterministic string hash of the org id → hue `0..359`; fixed S/L tuned for
  the dark rail (e.g. `65% 55%`). Return `"<h> <s>% <l>%"` (HSL-triple, same
  format Project tiles feed into `hsl(...)`).

## Step 3 — Extend `AppBarOrgTile`
`packages/ui/src/components/AppBarOrgTile.tsx`
- Add optional props `expanded?: boolean`, `onToggleExpanded?: () => void`.
- Paths:
  - 0 orgs → `null` (unchanged).
  - 1 org → single static tile, no toggle (unchanged).
  - >1 org + collapsed → active org tile + a small caret-down toggle button
    (`aria-expanded={false}`, `onClick={onToggleExpanded}`). Replaces the old
    dropdown as the default.
  - >1 org + expanded → an `Orgs` section label (matching `AppBar`'s
    `AppBarSectionLabel` look) + a vertical list of all org tiles rendered in
    the **project-tile recipe**: `getOrgInitials`, `w-10 h-10 rounded-lg`,
    right-side `Tooltip` with the org name, `onClick={() => onSelect(id)}`;
    selected tile gets inline `style={{ color: 'hsl(<c>)', backgroundColor:
    'hsl(<c> / 0.2)' }}` with `c = getOrgColor(id)`, non-selected get
    `bg-primary text-normal hover:opacity-80`. Caret-up collapse toggle at end.
- Keep exports/`AppBarOrgTileOrganization` type stable.

## Step 4 — Wire up in the cloud shell
`packages/remote-web/src/app/layout/RemoteAppShell.tsx`
- Import `useOrgRailStore`; read `expanded` + `toggleExpanded`.
- Pass `expanded={expanded}` and `onToggleExpanded={toggleExpanded}` into the
  `<AppBarOrgTile>` rendered as `orgSlot`. Nothing else changes — selecting an
  org already re-scopes projects via `activeOrganizationId` → `projectsQuery`.

## Step 5 — Tests
- Add/extend a Vitest for `AppBarOrgTile`: (a) 1 org → no toggle button;
  (b) >1 org collapsed → toggle present, list hidden; (c) expanded → one tile
  per org rendered and clicking a tile calls `onSelect(id)`.

## Step 6 — Verify
- `pnpm run check`, `pnpm run lint`, `pnpm run format`.
- Manual/verify: expand → tiles appear above projects; click switches org and
  the project list updates; state persists across reload.

## Step 7 — Codex review
- Run the `codex-review` skill / Codex CLI on the diff; iterate ≤3 times;
  address confirmed findings and re-verify.

## Rollback
Revert the three files + delete the new store file. No data/schema/API impact.
