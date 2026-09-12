# Implementation Plan: Desktop Deploy Status

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Frontend: React and TypeScript with Tailwind classes.
- Composition: `packages/web-core/src/pages/workspaces/RightSidebar.tsx` owns
  the persistent desktop workspace right drawer and its ordered section stack.
- Data: `packages/web-core/src/shared/hooks/useUserSystem.ts` already exposes
  `appVersion` and `deploymentTimestamp` from the existing `/api/info` load.
- Presentation: `packages/ui/src/components/DeployStatus.tsx` already owns
  revision linking, development fallback, elapsed-age formatting, timer refresh,
  and accessible labels.
- Tests: Vitest/Testing Library coverage exists in
  `packages/remote-web/src/app/layout/Navbar.test.tsx`; new drawer composition
  coverage should live with the workspace frontend surface if an established
  test harness exists, otherwise use the narrowest shared rendered-DOM harness.
- Constraints: no backend, generated-contract, dependency, persisted preference,
  or homelab change is needed.

## Architecture & Approach

1. Extend `DeployStatus` with an additive display-density option only if needed
   so desktop can show its age at normal drawer widths while its current compact
   mobile responsive behavior remains the default.
2. In `RightSidebar.tsx`, consume `appVersion` and `deploymentTimestamp` through
   `useUserSystem`, matching the established context boundary rather than
   fetching system info again.
3. Add a small fixed row as the first child of the drawer's divided flex stack.
   The row uses `flex-none`/`shrink-0` intrinsic sizing and does not use
   `CollapsibleSectionHeader`, `usePersistedExpanded`, `PERSIST_KEYS`, or any
   action definition. It contains the `Deploy Status` label and shared status
   presentation.
4. Leave the existing `sections` construction and map unchanged so all current
   persisted expansion behavior, equal remaining-height allocation, and body
   overflow ownership continue to work.
5. Add focused tests for desktop row order/structure and any additive shared
   presentation option. Retain the existing mobile suite as regression coverage.

## Data Model

No new stored or transported model. The view consumes two existing optional
values:

| Value | Source | Desktop rule |
| --- | --- | --- |
| `appVersion` | Existing user-system context | Render revision; real SHA links, `dev` does not. |
| `deploymentTimestamp` | Existing user-system context | Derive age when valid; omit age otherwise. |

No value is persisted by this feature.

## Contracts

No API or generated-type contract changes. The only component-interface change,
if necessary, is an optional backwards-compatible presentation prop on
`DeployStatus`; its current behavior remains the default.

## Research Notes

See `./research.md`.

## Constitution Check

- **II — Test the contract**: rendered-DOM coverage protects placement,
  non-collapsibility, and shared status behavior.
- **III — Small, reversible steps**: the plan adds one composed row and reuses
  the current metadata/context/component path.
- **IV — Shared-component boundaries are law**: `web-core` owns the feature
  composition/data consumption; `packages/ui` continues to own deploy-status
  presentation.
- **VI / XXI — Don't rebuild what shipped; one convention per concept**: no
  second formatter, timer, link rule, API request, or deployment-age meaning.
- **XIV — Repository verification is worktree-safe**: locked pnpm setup precedes
  formatting and verification.
- **XXVI — Collapsed controls retain decisive context**: no new expandable
  control is introduced; existing drawer section headers are unchanged.

No constitution deviation is required.

## Risks & Dependencies

- A row accidentally made flexible could steal height from expanded sections;
  use explicit intrinsic sizing and protect it in rendered markup tests.
- The mobile component currently hides age below a viewport breakpoint; desktop
  reuse may need an explicit density option rather than overriding internals with
  brittle descendant selectors.
- `RightSidebar` has several context dependencies, so test setup may be costly.
  Prefer the repository's existing workspace layout harness; if none exists,
  extract only a stateless row presentation without moving data ownership out of
  `RightSidebar`.
- Both local and remote frontends consume shared packages, so verification must
  cover their TypeScript blast radius.
