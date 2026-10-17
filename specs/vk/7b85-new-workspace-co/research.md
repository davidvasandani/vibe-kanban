# Research Notes

## Decision 1: fix the height chain, not the scroll position

**Chosen:** declare the full flex containment contract from
`CreateChatBoxContainer` down through `ChatBoxBase`, with the prompt area as the single
scroll owner.

**Rejected — make the page scroll.** Both hosts clip
(`WorkspacesLayout.tsx:319`/`:476`, `ProjectRightSidebarContainer.tsx:137`), so page-level
scrolling would mean unwinding the app's panel model. `nested-flex-scroll-containment`
records this as a rejected blast-radius increase on a prior task of the same shape.

**Rejected — sticky/fixed config row.** It keeps the row on screen by taking it out of
flow, which produces a second scroll surface and divergent behaviour between the panel and
the full-page hosts. Also explicitly rejected in the prior task, and now ruled out by
constitution XXXVII.

**Rejected — measure the panel in JavaScript and set a pixel max-height.** Reintroduces a
height authority the host already owns, adds resize observers, and is the other thing
`nested-flex-scroll-containment` warns against alongside viewport units.

## Decision 2: remove `max-h-[50vh]` rather than tune it

`50vh` is a viewport fraction, but the composer's real budget is the panel, which in the
project sidebar is a fraction of the viewport already reduced by the navbar and the panel
header. Any constant is wrong for some host. Tuning the number would move the failure
threshold rather than remove it. Replacing it with flex sizing against the container makes
the budget exact in every host — FR-9.

Secondary benefit: the class is applied to both the `ContentEditable`
(`WYSIWYGEditor.tsx:532`) and the absolutely-positioned placeholder (`:501`). Dropping the
overflow/cap from that class stops a decorative node from being given a scroller.

## Decision 3: scroll owner is a slot wrapper, not the `ContentEditable`

To make the `ContentEditable` itself the flexible body, `WYSIWYGEditor`'s intermediate
`wysiwyg` and `relative` wrappers (`WYSIWYGEditor.tsx:513`, `:528`) would have to become
flex containers. Those wrappers are shared by every editor instance in the app, including
read-only renderers, so changing them has a far larger blast radius than the defect.

Owning the scroll in a wrapper that `ChatBoxBase` renders keeps the change inside the chat
box, and satisfies the knowledge base's "keep scroll ownership at one level".

## Decision 4: `fillHeight` opt-in rather than always-on

`ChatBoxBase` has two consumers. `SessionChatBox` sits at the bottom of a conversation and
must stay intrinsically sized — making the base always fill would change that surface for
no reason and risks the conversation scroller fighting the composer. An opt-in boolean
keeps one implementation of the layout (constitution IV) while letting the two consumers
differ where they genuinely differ. FR-8 is then directly testable: assert the default mode
carries none of the fill-mode classes.

## Decision 5: keep `items-center` and add `max-h-full`

Centring is the current look and FR-7 requires preserving it. `align-items: center` with no
cap is what lets a tall child overflow past both edges; `max-height: 100%` caps the child
to the container so a tall column fits exactly while a short one still centres. This keeps
the visual change at zero for the common case.

## Decision 6: structural coverage, in the `remote-web` lane

JSDOM computes no layout, scroll height, or overflow. Every prior task of this shape
(`nested-flex-scroll-containment`, `responsive-flex-toolbars`) asserted the structural
relationship instead, and both note that browser evidence is still needed for the pixel
outcome. `packages/remote-web/src/test/` is the established lane for rendering `@vibe/ui`
components (`SessionChatBox.test.tsx`), and using it also exercises the remote frontend's
half of the constitution IV blast radius.

## Dependencies

None added. No generated files, Rust crates, migrations, or API contracts are touched.
