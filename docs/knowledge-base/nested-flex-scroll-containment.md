# Nested flex scroll containment

Tags: `vk/4f69-vk-create-issue`

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

The host still needs to provide a definite height. Keep scroll ownership at one
level: the shell clips, the header does not shrink, and only the body scrolls.
Avoid duplicating host height with viewport units or JavaScript measurement when
the existing height chain is already definite.

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

## Regression pattern

JSDOM does not calculate layout, scroll height, or actual pixel overflow. A
rendered-component test can still deterministically protect the browser-relevant
contract by asserting:

- shell classes include the column/height/clipping constraints;
- body classes include `min-h-0`, flexible growth, and vertical auto overflow;
- lower controls are descendants of the body rather than siblings outside the
  scroll region.

Use stable selectors for the shell and scroll region instead of brittle child
indexes. Pair this test with a pre-fix failure demonstration and manual visual
verification when a browser/device runner is available.

## Verification used for this task

- Focused `KanbanIssuePanel` Vitest: 6 tests passed.
- Shared UI and remote-web TypeScript checks passed.
- Shared UI ESLint passed.
- Repository formatting and `git diff --check` passed.
- Independent Codex CLI review reported no blocking correctness issues.
