# Implementation Plan: New workspace config never falls below the fold

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
- TypeScript + React 18, Tailwind (`packages/local-web/tailwind.new.config.js`, design
  tokens documented in `packages/local-web/AGENTS.md`).
- Shared tiers per constitution IV: `packages/ui` (`@vibe/ui`) owns presentational layout;
  `packages/web-core` owns feature containers. Both frontends (`local-web`, `remote-web`)
  are the blast radius.
- Test lane: Vitest + Testing Library. Shared `packages/ui` components are covered through
  the established consumer lane at `packages/remote-web/src/test/` (see
  `SessionChatBox.test.tsx`), configured by `packages/remote-web/vitest.config.ts`
  (`environment: "jsdom"`). JSDOM computes no layout, so coverage is structural.
- No new dependencies. No Rust, DB, or generated-type changes.

## Architecture & Approach

### Where the height chain breaks today
`packages/web-core/src/shared/components/CreateChatBoxContainer.tsx` renders into hosts
that already supply a definite, clipped height:

- `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx:334` (mobile chat tab) —
  `flex-1 min-h-0 overflow-hidden` — and `:476` (desktop left `Panel`) —
  `min-w-0 h-full overflow-hidden`, a definite height by a different route;
- `packages/web-core/src/pages/kanban/ProjectRightSidebarContainer.tsx:138`
  (`WorkspaceCreatePanel` body) — `flex-1 min-h-0`. This is the panel in the report.

Inside that height, the container declares no shrink contract:

1. `CreateChatBoxContainer.tsx:334` root — `relative flex flex-1 flex-col bg-primary h-full`,
   no `min-h-0`, no clipping.
2. `:335` — `flex flex-1 items-center justify-center px-base`. With `align-items: center`
   and no cap, content taller than the box overflows past both edges; the bottom is
   unreachable because an ancestor clips.
3. `:418` — the prompt editor is capped `min-h-double max-h-[50vh] overflow-y-auto`. That
   `50vh` duplicates a host height that is already definite — the exact anti-pattern named
   in `docs/knowledge-base/nested-flex-scroll-containment.md` and now in constitution XXXVII.
   The class is also spread onto the Lexical placeholder
   (`packages/web-core/src/shared/components/WYSIWYGEditor.tsx:501`) as well as the
   `ContentEditable` (`:532`), so the scroller is duplicated onto a decorative node.
4. Nothing between the column and the config row is `shrink-0` or `min-h-0`, so the layout
   has no way to know the prompt is what should give way.

`packages/ui/src/components/ChatBoxBase.tsx` is intrinsically sized throughout: its root is
`flex ... flex-col` with no height contract, its editor area (`:126`) is a plain column, and
its footer (`:130`) — the config row — is an ordinary flex item.

### The change
Declare the full contract along the whole chain, move scroll ownership to one wrapper, and
delete the viewport-unit cap. Height behaviour is **opt-in** at the `packages/ui` boundary
so `SessionChatBox` (`packages/ui/src/components/SessionChatBox.tsx:758`), which is
intrinsically sized at the bottom of a conversation, is untouched — FR-8.

**1. `packages/ui/src/components/ChatBoxBase.tsx`** — add `fillHeight?: boolean`.
- Root: `fillHeight && 'min-h-0'` — the box can shrink inside its parent. Shrink-only, not
  `flex-1`: see the sizing note in `./contracts/layout-contract.md` §2.
- Error alert (`:106`), fill-mode banner box (`:113`) and header (`:116`) blocks:
  `shrink-0`, so a
  validation error or a state swap cannot take height from the config row — FR-5,
  constitution XXXVII ("asynchronous state may not change the height budget").
- Editor area (`:126`): `fillHeight && 'min-h-0'`.
- Wrap the injected `editor` node in a slot `div` that, when filling, carries
  `min-h-0 overflow-y-auto` and is the single scroll owner — FR-2, FR-3, FR-10. The
  wrapper is rendered **unconditionally** so the flex-child count of the editor area, and
  therefore `gap-plusfifty`, is identical in both modes; in the default mode it carries no
  classes and is layout-transparent.
- Footer controls row (`:130`): `shrink-0` — FR-1, FR-4.
- Add stable `data-testid`s (`chat-box-editor-slot`, `chat-box-footer`) for the structural
  test; the knowledge base requires stable selectors over child indexes.

**2. `packages/ui/src/components/CreateChatBox.tsx`** — accept `fillHeight` and forward it
to `ChatBoxBase`. Purely additive; existing callers are unaffected.

**3. `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`**
- Root (`:334`): add `min-h-0 overflow-y-auto` — the shell owns overflow (FR-3). It scrolls
  rather than clips so that below the supported viewport range the create action stays
  reachable instead of being discarded; at supported sizes it never has scrollable overflow.
- Centring wrapper (`:335`): add `min-h-0` so it can shrink within the root.
- Content column (`:336`): add `max-h-full min-h-0`. `max-h-full` resolves because the
  parent's height is definite. With `items-center` retained, a tall column is capped to
  exactly the available height instead of overflowing past both edges, and a short column
  stays vertically centred exactly as today — FR-7.
- `shrink-0` on the linked-issue warning (`:338`) — passed as a `className` onto the
  advisory’s own root rather than a wrapper, because the advisory renders `null` when there
  are no siblings or it has been dismissed and a wrapper would leave an empty flex child
  contributing a stray `gap-base`. Also `shrink-0` on both step headings (`:346`, `:357`) and
  the placement row (`:364`) — FR-4, FR-11.
- `min-h-0` on the `@container` wrapper (`:361`) and on the inner column it holds (`:362`), so the
  deficit is routed through to the editor slot rather than stalling on an intermediate
  item's automatic minimum — the zero-minimum rule from both knowledge-base pages.
- Repository step: give the picker region `min-h-0` and its own vertical overflow so that
  step stays usable under the same cap — FR-6. The steps are mutually exclusive, so this is
  still one scroll owner per rendered screen.
- Pass `fillHeight` to `CreateChatBox`, and change the editor class (`:418`) from
  `min-h-double max-h-[50vh] overflow-y-auto` to `min-h-double`. The slot now owns
  scrolling, and the placeholder no longer inherits a scroller.

**4. `packages/remote-web/src/test/CreateChatBox.test.tsx`** (new) — render the real shared
component in the established lane and assert the structural contract, per constitution II
and XXXVII:
- in fill mode the root is shrinkable; the editor slot has a zero minimum and owns
  vertical overflow; the footer config row is `shrink-0`;
- the injected editor is a **descendant of the slot** while the config row is **not**, so
  scrolling the prompt can never carry the config row out of view;
- a validation error and the in-flight create state both leave the footer present and
  `shrink-0`;
- by default none of the fill-mode classes appear, protecting `SessionChatBox` (FR-8).

## Data Model
Not applicable — no entities, persistence, or API surface change.

## Contracts
See `./contracts/layout-contract.md` for the flex containment contract this feature
establishes and the `ChatBoxBase` prop contract that carries it.

## Research Notes
See `./research.md`.

## Constitution Check
Checked against `.specify/memory/constitution.md` v0.32.0.

- **II. Test the contract** — honored. Acceptance criteria are written before
  implementation; a rendered-DOM component test is added in the existing shared-UI lane.
- **III. Small, reversible steps / VI. Don't rebuild what shipped** — honored. No new
  component, no new dependency, no restructuring of the composer; the change is a set of
  layout classes plus one optional boolean prop, and it reuses the exact contract already
  proven in `KanbanIssuePanel`.
- **IV. Shared-component boundaries are law** — honored. The chat box's internal layout is
  fixed in `packages/ui`, not duplicated into route containers; `web-core` continues to
  supply data and host sizing. Both frontends are treated as the blast radius, and the test
  runs in the `remote-web` lane for that reason.
- **XXXVII. Committing controls stay inside the host's height** — this is the feature. The
  full contract is declared (shell clips, fixed rows `shrink-0`, zero minimums on the path,
  one overflow owner); the `50vh` duplication of an already-definite host height is removed;
  error and in-flight states are made non-authoritative; the behaviour is opt-in at the
  shared component; coverage is structural rather than pixel-based.
- **XXXVI. Dynamic viewports have one scroll authority** — not in conflict. That principle
  governs which policy corrects scroll position in a live view; this change adds a single
  static scroll owner in a non-live surface and does not introduce a competing corrector.
- **Constraints** — no new dependency, no generated-file edits, `pnpm run format` before
  completion.

No deviation to record.

## Risks & Dependencies
- **`max-h-full` under `items-center`.** The cap resolves only while the parent's height is
  definite. The sidebar panel host (`ProjectRightSidebarContainer.tsx:138`) supplies
  `flex-1 min-h-0` beneath an `h-full` chain and the desktop left `Panel`
  (`WorkspacesLayout.tsx:476`) supplies `h-full overflow-hidden`, so the cap resolves through
  two different chains and each needs its own check. The sidebar host clips nothing itself,
  which the container's own new overflow ownership now covers. Mitigation: the
  container-level half of the chain is verified by code inspection and browser check rather
  than by a unit test. `CreateChatBoxContainer` pulls in create-mode, Electric, auth,
  dropzone and executor-config hooks, so rendering it would test the mock harness more than
  the layout; the shared-component half, where the contract actually lives, is unit-covered.
- **Moving the scroller off `ContentEditable`.** Lexical's floating toolbar and the absolute
  placeholder sit inside `WYSIWYGEditor`'s own `relative` wrappers, so scrolling one level
  out should be transparent — but this needs browser confirmation, not just JSDOM.
- **Unconditional editor slot wrapper.** A new block-level div between the editor area and
  the editor node. Layout-transparent in the default mode, but it does change the DOM for
  `SessionChatBox`; the test asserts the default mode carries no fill-mode classes, and the
  existing `SessionChatBox.test.tsx` must keep passing.
- **JSDOM proves nothing about pixels.** Structural coverage plus manual/browser
  verification of the narrow-viewport case is required before the task is called done.
- Depends on `pnpm install --frozen-lockfile` in this fresh worktree before verification.
