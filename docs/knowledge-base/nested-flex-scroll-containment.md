# Nested flex scroll containment

Tags: `vk/4f69-vk-create-issue`, `vk/7b85-new-workspace-co`

## Boundary rule

In a height-constrained column flex layout, declaring a child
`overflow-y-auto` does not by itself guarantee that the child will scroll. A
flex item's automatic minimum block size can remain content-sized, preventing
it from shrinking into the space left by fixed siblings. If an ancestor also
clips overflow, the lower content is cut off instead of becoming reachable.

For a fixed header plus scrolling body, establish the complete contract:

```text
shell: flex flex-col h-full overflow-hidden
header: shrink-0
body: min-h-0 flex-1 overflow-y-auto
```

The host still needs to provide a definite height. Keep working scroll ownership
at one level: the shell owns overflow, the header does not shrink, and only the
body scrolls. Avoid duplicating host height with viewport units or JavaScript
measurement when the existing height chain is already definite.

Two refinements from the create-composer application below:

- **The shell should scroll, not clip, when clipping would hide a committing
  control.** `overflow-hidden` plus `shrink-0` on every fixed row means that
  once the flexible body reaches its minimum, any further deficit is *discarded*
  — and the thing discarded is whatever sits last in the column, which is
  usually the submit action. `overflow-y-auto` costs nothing in the normal case
  (there is no scrollable overflow) and converts the pathological case from
  "unreachable" to "one scroll away". This is still one *working* scroll owner:
  the shell only ever engages outside the supported size range.
- **The flexible body needs a floor; the zero minimum belongs above it.**
  `min-h-0` is load-bearing on every *intermediate* item, because their
  automatic minimum block size is what stalls the deficit. Putting `min-h-0` on
  the terminal body too lets it collapse to zero, which trades a hidden footer
  for an invisible editor. Give the body a small explicit floor instead.

## Vibe Kanban issue-panel application

`packages/ui/src/components/KanbanIssuePanel.tsx` is shared by local and remote
frontends and by create/edit modes. Its mobile and desktop hosts already provide
`h-full overflow-hidden`. The panel already had a fixed header and a body marked
`flex-1 overflow-y-auto`; adding `min-h-0` to that body made the intended
scrolling effective without moving pipeline settings, the draft-workspace
toggle, the Create Issue action, or edit-mode sections.

Prefer fixing this at the shared presentational boundary. Mode-specific
wrappers, sticky submit actions, or application-wide viewport changes increase
the blast radius and can create divergent scroll behavior.

## Vibe Kanban create-composer application

`CreateChatBoxContainer` renders into three hosts that all supply a definite,
clipped height (`WorkspacesLayout`'s mobile chat tab and desktop left `Panel`,
and `ProjectRightSidebarContainer`'s `WorkspaceCreatePanel` body). It declared
none of the contract: the root had no zero minimum, content was centred with
`flex-1 items-center justify-center` so a tall column overflowed past *both*
edges, and the prompt editor was capped `max-h-[50vh]` — a viewport fraction
standing in for a panel height the host had already made definite. With a long
prompt on a phone the config row and the Create button fell below the fold with
no way to scroll to them.

Two further lessons specific to this shape:

- **Move the scroller to a wrapper you own rather than restructuring the
  editor.** The cap lived on the Lexical `ContentEditable`'s `className`, which
  is also spread onto the absolutely-positioned placeholder. Making the
  `ContentEditable` itself the flexible body would have meant turning
  `WYSIWYGEditor`'s internal `wysiwyg`/`relative` wrappers into flex containers,
  which every editor instance in the app shares. A slot `div` rendered by the
  chat box keeps the blast radius inside the chat box. Check portalled overlays
  when you move a scroller: Lexical's floating toolbar and typeaheads portal to
  `document.body` and the toolbar's scroll listener is capture-phase, so they
  survive a new inner scroller.
- **Prefer shrink-only sizing to `flex-1` under an `items-center` +
  `max-h-full` column.** `flex-1` sets `flex-basis: 0`, which makes the column's
  intrinsic height depend on the flex fraction algorithm instead of on content.
  A zero minimum plus the default `flex-shrink: 1` and a content-derived basis
  gives the same yielding behaviour while keeping the short-content case
  pixel-identical to the pre-change centred layout. `max-h-full` on the column
  is what stops a tall child overflowing past both edges of a centred row; it
  resolves as long as the parent's height is definite.

## Opt-in height behaviour at a shared boundary

`ChatBoxBase` is shared by the create composer and the session composer, and
only the first is height-constrained — the session composer sits at the bottom
of a conversation and must stay intrinsically sized. An optional `fillHeight`
prop keeps one implementation of the layout while letting the two consumers
differ where they genuinely differ, and makes the isolation directly testable
("the default mode carries none of the fill-mode classes").

Two traps when adding such a prop:

- A wrapper added *unconditionally* keeps the flex-child count stable, so gaps
  do not shift — but a wrapper around a child that may render `null` leaves an
  empty flex element behind, and in a gapped column that is visible blank space.
  For a component that can render nothing, pass a `className` onto its own root
  instead of boxing it.
- Every row the prop's documentation claims to protect must actually be
  protected by the component, not by a caller-side invariant that nothing
  enforces. If a slot is rendered bare today only because no filling caller
  passes it, box it in the filling branch anyway.

## Regression pattern

JSDOM does not calculate layout, scroll height, or actual pixel overflow. A
rendered-component test can still deterministically protect the browser-relevant
contract by asserting:

- shell classes include the column/height/overflow-ownership constraints;
- intermediate items carry a zero minimum, and the body carries its floor and
  vertical auto overflow;
- fixed rows carry `shrink-0`, so the body is the only shrink target;
- lower controls are descendants of the body rather than siblings outside the
  scroll region — or, for a footer that must stay put, the inverse: the controls
  are *not* inside the scroll region.

Use stable selectors for the shell and scroll region instead of brittle child
indexes. Pair this test with a pre-fix failure demonstration and manual visual
verification when a browser/device runner is available.

## Verification used for these tasks

`vk/4f69-vk-create-issue`:

- Focused `KanbanIssuePanel` Vitest: 6 tests passed.
- Shared UI and remote-web TypeScript checks passed.
- Shared UI ESLint passed.
- Repository formatting and `git diff --check` passed.
- Independent Codex CLI review reported no blocking correctness issues.

`vk/7b85-new-workspace-co`:

- New `CreateChatBox` and `ChatBoxBase` fill-height Vitest, plus advisory
  regression coverage; full `remote-web` suite 83 passed, `web-core` 510 passed.
- Pre-fix failure demonstrated: neutralising `fillHeight` failed 7 of the 10 new
  structural assertions.
- All four frontend TypeScript checks, both ESLint lanes, the i18n key check,
  `cargo check`/`clippy` on both Rust workspaces, `pnpm run format` and
  `git diff --check` passed.
- Independent review: the Codex CLI could not run (its stored `refresh_token` is
  an empty string, so token refresh 400s), so the repository's `/code-review`
  skill was substituted at high effort and run three times until it reported no
  correctness bug in the flex chain. Six findings across the first two rounds
  and four in the third were all addressed.
- **Browser verification was not possible**: the workspace browser MCP upstream
  was unavailable for the whole task. JSDOM computes no layout, so the
  pixel-level outcome on a narrow viewport remains unconfirmed by this task.
