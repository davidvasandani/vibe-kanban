# Technical Plan: Scrollable Create-Issue Settings

**Feature dir**: `specs/vk/4f69-vk-create-issue/`  
**Task**: `vk/4f69-vk-create-issue`  
**Spec**: [`spec.md`](spec.md)  
**Status**: Draft

## Technical Context

The affected surface is the React/TypeScript shared UI package. Tailwind utility
classes define layout. `KanbanIssuePanel` is rendered by the shared web-core
container and used by local and remote frontends. Existing remote-web Vitest +
Testing Library coverage renders the component directly.

The panel hosts already provide a definite height and clipped overflow. The
panel itself is a full-height flex column with a fixed header and a flexing body.
The body's vertical overflow is already `auto`, but its automatic flex minimum
size can prevent it from shrinking inside the panel.

## Architecture & Approach

1. Extend `packages/remote-web/src/test/KanbanIssuePanel.test.tsx` with a
   regression test that identifies the panel shell/body using stable test IDs,
   asserts the shell's height/flex/overflow contract, asserts the body's
   `min-h-0 flex-1 overflow-y-auto` contract, and proves the create settings and
   Create Issue action are descendants of that body.
2. Update `packages/ui/src/components/KanbanIssuePanel.tsx` only:
   - add stable test IDs to the shell and scroll region;
   - give the body `min-h-0` while retaining `flex-1 overflow-y-auto`;
   - retain shell `h-full overflow-hidden`, fixed header, DOM order, and all
     event/state behavior.
3. Avoid mode-specific wrappers, JavaScript height measurement, sticky actions,
   viewport calculations, or host-level layout changes.

This maps FR-1 through FR-5 to one shared layout correction and FR-7 to the
rendered component test. FR-6 is protected by leaving component logic and DOM
ordering unchanged.

## Data Model

See [`data-model.md`](data-model.md). No data model or persistent state changes.

## Contracts

See [`contracts.md`](contracts.md). No HTTP/API contract changes; the only
changed contract is the internal rendered layout contract.

## Research Notes

See [`research.md`](research.md). No new dependency is required.

## Constitution Check

- **I — Clarity over cleverness:** a standard `min-h-0` flex constraint makes
  the existing intended scroll ownership effective.
- **II — Test the contract:** rendered-DOM coverage asserts the exact shell/body
  layout contract and containment of create controls.
- **III — Small, reversible steps:** two frontend files change; rollback is a
  class/test-ID/test removal.
- **IV — Shared-component boundaries are law:** layout remains in
  `packages/ui`; `web-core` data behavior is untouched. Both local and remote
  frontends receive the shared fix.
- **VI — Don't rebuild what shipped:** the existing scroll region is corrected,
  not replaced.
- **XIV — Repository verification is worktree-safe:** run the frozen dependency
  install before mandated formatting when needed.
- Other principles are not applicable because there are no backend mutations,
  external protocols, destructive operations, managed tools, or persistence.

No constitution deviation or open question remains.

## Risks & Dependencies

- JSDOM cannot calculate real overflow. The test therefore locks the CSS
  contract whose browser behavior is standard, while the supplied screenshot
  and manual responsive verification remain visual evidence.
- Stable test IDs add non-user-facing DOM attributes. They are preferred over
  brittle child-index selectors for distinguishing the shell and scroll body.
- Full repository lint/check may be expensive; focused tests are mandatory and
  relevant package checks run before broader verification.

## Rollback

Revert the new `min-h-0` utility, test IDs, and regression test. No schema,
generated type, API, persistent state, or deployment rollback is required.
