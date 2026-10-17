# Research — vk/2b39-fix-mobile-ui

## R1 Layout options (see `mockups/`)
| Option | Description | Verdict |
|---|---|---|
| 0 Current | One line. The right cluster is `shrink-0` and overlaps the id. | Broken |
| **A Two-line row** | Line 1: id, status, title, age. Line 2: tags, relationships, assignee (clipped). | **Chosen.** Keeps all the context and has a predictable height (two lines). Small diff. |
| B Compact single line | One line: id, status, title, age, plus a counter chip (e.g. "⛔1 · 🏷2") | Densest, but hides *which* issue blocks this one. Rejected per user story 2. |
| C Stacked card | A card per issue: title on its own line, then meta | Most readable, but about 3× taller, and it duplicates the board view. Rejected. |

## R2 Mobile signal
Use the `isMobile` prop from `useIsMobile()` instead of Tailwind `sm:`/`md:`
variants. The `ui` package's mobile variants (`KanbanFilterBar`,
`KanbanCardContent`, `KanbanBoard`) already take this prop, and it keeps one
layout authority (wiki: workspace-context-bar-responsive-visibility).

## R3 Clip vs wrap for line 2
Clip. Wrapping makes row height depend on the data. A badge cut off at the
row edge still reads as "there's more", and the issue panel shows it all.

## R4 Testing
`packages/ui` has no test runner. `IssueListRow` depends on a `Draggable`
context, so a DOM test would need a dnd harness that no other ui test uses.
Validation is TypeScript, ESLint, and a 390px browser render.

## R5 Known, out of scope
Hover-gated checkbox swapping on touch (tap triggers `:hover`) predates this
change. It is left alone.
