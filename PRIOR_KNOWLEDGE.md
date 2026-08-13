# Prior Knowledge: Single-Value Browser Titles

The project knowledge base is not empty. One page is directly relevant and a
second establishes the boundary between browser titles and visible issue UI.

## `wiki/workspace-navbar-breadcrumbs.md`

- Workspace breadcrumbs are deliberately hierarchical and are assembled in
  web-core before the shared navbar renders them.
- A linked issue's internal UUID is never a display label. Once resolved, its
  `simple_id` (for example `VK-123`) is the correct compact label for the
  *visible breadcrumb*.
- The breadcrumb models async relationship resolution explicitly and should not
  be changed as a side effect of browser-title work.
- Consequence: the ticket number remains useful in visible navigation, but that
  does not justify concatenating it into `document.title`.

## `wiki/kanban-issue-panel-sections.md`

- Issue identity can appear in visible issue-detail chrome, including the
  panel's header id group.
- The panel is shared between local and remote web surfaces, and its visual
  composition is separate from page-level browser metadata.
- Consequence: this task should not remove ticket identifiers from issue cards,
  issue panels, or external-integration badges.

## Source Inspection Relevant to the Spec

- `packages/web-core/src/shared/hooks/usePageTitle.ts` currently filters all
  supplied parts, joins them with ` - `, and appends ` | Vibe Kanban`.
- `ProjectKanban.tsx` supplies both the open issue title and project name, while
  workspace pages supply one workspace/create-mode label.
- There is no knowledge-base page describing a browser-title selection
  contract, so the shipped behavior should be recorded if it proves reusable.

## Consequences for This Task

1. Treat hook arguments as an ordered fallback chain, not title fragments.
2. Preserve `simple_id` in visible breadcrumbs and other issue identity UI.
3. Keep browser-title changes localized to web-core metadata behavior and its
   call sites/tests.
4. Use `Vibe Kanban` only when every page-specific fallback is absent.
