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

## Apply the contract to every step, not just the loudest one

The reported symptom was the prompt step, and fixing it there is what the change
was reviewed on. The repository step got a scroller too — but the whole picker
bar went inside it, including the **Continue** button that commits that step, so
Continue scrolled out of view exactly like the footer had. Structural tests
passed, three rounds of code review passed, and the defect survived all of them
because every reviewer was looking at the step named in the report.

Enumerate the surface's committing controls first, then check each one against
the contract. Here that is Create on the prompt step and Continue on the
repository step; wrapping a component that contains its own submit action in a
scroller silently demotes that action.

The same inconsistency then repeated one level down: fixing the repository step
re-applied the contract to its list and controls row but initially skipped its
error row (no `shrink-0`) and its list floor (`min-h-0` with nothing to stop a
collapse to zero) — both of which the prompt step had already got right. When a
contract is applied to a second surface, walk the *whole* checklist against it
rather than porting the parts that fixed the reported symptom.

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
- Browser verification: initially blocked (the workspace browser MCP upstream was
  down), completed later in headless Chrome. Two techniques worth reusing:
  - Build the fixture from the components' **verbatim class strings** and compile
    the repository's own Tailwind config against it, so the test exercises the
    real utility definitions instead of hand-written CSS approximations. Parsing
    the class strings back out of the source keeps the fixture from drifting.
  - Better still, mount the **real components**: a throwaway `harness.html` plus
    entry module in `packages/local-web` gets the app's own Vite config, aliases
    and virtual modules for free, so the actual Lexical editor renders. This is
    what proved scroll ownership genuinely left the `ContentEditable`
    (`max-height: none`, `overflow-y: visible` on it, the slot scrolling
    instead) and that the placeholder no longer inherits a scroller — neither is
    observable from a reconstruction. **Import the app's i18n setup**: without
    it `t()` returns raw keys, and a key like
    `tasks:conversation.workspace.create` is several times the width of
    "Create", which wraps the toolbar and silently inflates every height you
    measure. Reconstruction and real-component runs agreed to within ~7px once
    i18n was loaded, and disagreed by ~55px without it.
  - When the managed browser is unavailable, Playwright's cached Chromium can be
    driven directly over CDP. On NixOS its dynamic libraries resolve by
    iteratively reading each `error while loading shared libraries: X` and
    appending the matching `/nix/store/*/lib` to `NIX_LD_LIBRARY_PATH`.
- Measured outcome against the real component tree: the config row and create
  action are fully visible down to **316px of host height** — at 320px with the
  prompt at its 48px floor, and at 310px they sit 6px low while the shell is
  scrollable by exactly 6px (then 16/21/26/36px at 300/295/290/280px). The
  prompt is 323px at a 600px host and 103px at 380px; short and empty prompts
  are unchanged and still centred. Knowing the threshold as a number is worth
  the effort: it converts "outside the supported range" from an assertion into
  a figure you can check a host against.
- **Browser measurement found a defect that JSDOM, three code-review rounds and
  the structural tests all missed** — the repository step's Continue action,
  30px below the host. Structural coverage proves the relationships you thought
  to assert; it cannot tell you which control you forgot.
