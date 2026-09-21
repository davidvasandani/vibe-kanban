# Implementation plan — new workspace config must never be below the fold

## Diagnosis

The create-workspace composer is rendered by
`packages/web-core/src/shared/components/CreateChatBoxContainer.tsx` into two hosts that
both supply a definite, clipped height:

- `WorkspacesLayout` mobile chat tab and desktop left panel (`flex-1 min-h-0 overflow-hidden`);
- `WorkspaceCreatePanel` in `ProjectRightSidebarContainer` (`flex-1 min-h-0`) — the
  "VAS-640 / Create Workspace" panel in the reported screenshots.

Inside that definite height the composer has no shrink contract at all:

1. the container root is `flex flex-1 flex-col h-full` with no `min-h-0` and no clipping;
2. the content is centred by `flex flex-1 items-center justify-center`, so when content is
   taller than the box it overflows past **both** edges and the bottom is unreachable;
3. the prompt editor is capped with `max-h-[50vh]` — a viewport unit duplicating a host
   height that is already definite, which `nested-flex-scroll-containment` explicitly
   warns against;
4. no element between the column and the config row is marked `shrink-0` or `min-h-0`, so
   nothing tells the layout that the editor is the part that should give way.

With a long prompt on a phone the heading, the "Run on" row, the chat header and a 50vh
editor exceed the panel, and the config row (model/preset selector, attach, repository
summary, linked-issue badge) plus the Create action fall below the fold.

## Approach

Apply the knowledge base's three-part containment contract down the whole chain, move the
single scroll owner to a wrapper we control, and delete the viewport-unit cap. Height
behaviour is opt-in at the shared `packages/ui` boundary so `SessionChatBox`, which is
intrinsically sized at the bottom of a conversation, is unaffected.

## Steps

1. **`packages/ui/src/components/ChatBoxBase.tsx`** — add an optional `fillHeight` prop.
   - Root: add `min-h-0` when filling, so the box can shrink inside its parent.
   - Error alert, banner and header: `shrink-0` so async error/state changes cannot take
     space from the config row (`edge-triggered-chat-scroll-intents`).
   - Editor area: `min-h-0` when filling. Sizing below the column is shrink-only — no
   `flex-1`, whose `flex-basis: 0` would make the column's intrinsic height depend on the
   flex fraction algorithm instead of on content, and content-derived bases are what keep
   the short-prompt case looking exactly as it does today.
   - Always wrap the injected editor node in a slot `div`; when filling, that slot carries
     `min-h-[3rem] overflow-y-auto` and becomes the single scroll owner and sole shrink
     target. The zero minimum belongs to the items above it; the slot itself stops at about
     two lines, because a composer shrunk to nothing is no more usable than a hidden footer. The wrapper is
     unconditional so the flex-child count, and therefore `gap-plusfifty`, is identical in
     both modes.
   - Footer controls row: `shrink-0` — this is the row that must stay visible.
   - Give the slot and the footer stable `data-testid`s for regression coverage.

2. **`packages/ui/src/components/CreateChatBox.tsx`** — accept `fillHeight` and pass it
   through. No behavioural change for existing callers.

3. **`packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`**
   - Root: add `min-h-0 overflow-y-auto`. The shell owns overflow but scrolls rather than
     clips, so a viewport below the supported range leaves the create action reachable
     instead of discarded.
   - Centring wrapper: add `min-h-0` so it can shrink within the root.
   - Content column: add `max-h-full min-h-0`. With `items-center` still in force, the cap
     means a tall column fits exactly instead of overflowing past both edges, while a short
     column stays vertically centred as today.
   - Mark the linked-issue warning, both step headings and the "Run on" row `shrink-0`, and
     add `min-h-0` to the `@container` wrapper and the column that hosts the chat box, so
     the deficit is routed to the editor slot rather than to the config row.
   - Repository step: allow the picker region to own its own overflow so that step stays
     usable under the same cap.
   - Pass `fillHeight` to `CreateChatBox` and replace the editor's
     `min-h-double max-h-[50vh] overflow-y-auto` with `min-h-double` only — the slot now
     owns scrolling, and the class no longer leaks a scroller onto the Lexical placeholder.

4. **Regression coverage** — add `packages/remote-web/src/test/CreateChatBox.test.tsx` in
   the established shared-UI consumer lane, rendering the real component and asserting the
   structural contract rather than geometry (JSDOM computes no layout):
   - root is shrinkable in fill mode;
   - the editor slot owns vertical overflow and keeps its floor, while the items above it
     carry a zero minimum;
   - the footer config row is `shrink-0`;
   - the injected editor is a descendant of the scroll slot while the config row is not,
     so scrolling the prompt can never carry the config row out of view;
   - none of the fill-mode classes appear by default, protecting `SessionChatBox`.

5. **Verification** — focused Vitest, `pnpm run check` (frontend + Rust workspaces),
   `pnpm run lint`, `pnpm run format`, `git diff --check`, then the independent Codex
   review, the knowledge-base update, and the pull request.

## Deliberately not doing

- No sticky/fixed positioning for the config row, no page-level scrolling, and no
  JavaScript height measurement — all rejected in prior tasks of this shape as blast-radius
  increases that produce divergent scroll behaviour.
- No change to `SessionChatBox` or its host; `fillHeight` defaults off.
- No reduction of safe-area padding to reclaim space.
