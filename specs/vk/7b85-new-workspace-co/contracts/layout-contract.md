# Contracts

## 1. Host contract (unchanged, relied upon)

A host of `CreateChatBoxContainer` supplies a definite height and does not grow with its
child:

```text
host: flex-1 min-h-0   (beneath an h-full chain)
```

Satisfied today by `WorkspacesLayout.tsx` (mobile chat tab, desktop left panel) and
`ProjectRightSidebarContainer.tsx` (`WorkspaceCreatePanel` body).

## 2. Containment contract (established by this feature)

```text
shell   (container root):  flex flex-col h-full flex-1 min-h-0 overflow-y-auto
centre  (centring row):    flex flex-1 min-h-0 items-center justify-center
column  (content column):  flex flex-col min-h-0 max-h-full
  heading / warning / placement row:  shrink-0
  composer region:                    min-h-0        (every intermediate flex item)
    chat box root:                    min-h-0
      error / banner / header:        shrink-0
      editor area:                    min-h-0
        editor slot:                  min-h-[3rem] overflow-y-auto     ← sole scroll owner
      footer config row:              shrink-0                          ← always visible
```

Sizing below the column is **shrink-only**: a zero minimum plus the default
`flex-shrink: 1` and a content-derived basis. Deliberately *not* `flex-1`, which
would set `flex-basis: 0` and make the column's intrinsic height depend on the flex
fraction algorithm rather than on content. Content-derived bases keep the short-prompt
case identical to today's centred layout (FR-7), and the deficit still lands
entirely on the editor slot because every sibling on the path is `shrink-0`. The prompt
area is not required to *grow* into spare space — only to yield space it does not have.

Invariants:

- exactly one element is the *working* scroll owner per rendered step: the editor slot on
  the prompt step, the picker region on the repository step. The two steps are mutually
  exclusive, so there is never a second active scroller on one screen;
- the shell also owns overflow, but only as a last resort. It scrolls rather than clips so
  that a viewport shorter than the screen's fixed cost leaves the create action reachable
  instead of discarded. At every supported size the shell has no scrollable overflow,
  because the column is capped at the host height and the editor slot absorbs the deficit;
- every flex item between `column` and the editor slot carries a zero minimum, so no
  automatic minimum block size stalls the deficit above the slot;
- the zero minimum belongs to the *intermediate* items, so the deficit reaches the slot.
  The slot itself keeps a small floor (`3rem`, about two lines): a composer shrunk to
  nothing is no more usable than a hidden footer, and past the floor the shell scrolls, so
  the footer stays reachable either way;
- the footer config row and the create action are never shrink targets;
- no length in this contract is expressed in viewport units.

## 3. `ChatBoxBase` prop contract

```ts
interface ChatBoxBaseProps {
  // ...existing props unchanged...

  /**
   * Fill and shrink within a height-constrained parent instead of sizing to
   * content. The editor slot becomes the sole scroll owner; header, banner,
   * error and footer rows stop shrinking. Off by default: SessionChatBox is
   * intrinsically sized.
   */
  fillHeight?: boolean;
}
```

`CreateChatBox` forwards the same optional prop with the same default.

Behavioural guarantees:

- `fillHeight` omitted or `false` ⇒ rendered classes are byte-identical to today's apart
  from the layout-transparent editor slot wrapper and the added `data-testid`s;
- `fillHeight` ⇒ the contract in §2 holds from the chat box root downwards.

## 4. Test selectors (stable, part of the contract)

| `data-testid`           | Element                                   |
| ----------------------- | ----------------------------------------- |
| `chat-box-editor-slot`  | scroll owner wrapping the injected editor |
| `chat-box-footer`       | footer config row                         |
