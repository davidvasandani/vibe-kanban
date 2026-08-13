# Browser title selection

Browser titles are owned by
`packages/web-core/src/shared/hooks/usePageTitle.ts`, which is shared by the
local and remote application routes. Its arguments are an **ordered fallback
chain**, not fragments to compose:

1. select the first candidate containing a non-whitespace character;
2. trim surrounding whitespace from that candidate; and
3. use `Vibe Kanban` only when no candidate is meaningful.

The selected label is the complete `document.title`. Do not append the product
name, project name, ticket/simple ID, separators, or other context. When a page
needs a loading fallback, express precedence through argument order—for example,
`usePageTitle(issue?.title, projectName)` selects the issue title once available
and otherwise uses the project name.

This metadata rule does not apply to visible navigation. Workspace breadcrumbs
remain hierarchical and retain their resolved issue `simple_id` under the
separate breadcrumb identity contract. A concise browser tab and an explicit
on-page hierarchy serve different purposes.

Focused hook coverage should exercise the real `document.title` effect,
including initial selection, whitespace-only candidates, all-absent fallback,
and rerender updates. A per-file jsdom Vitest environment is sufficient; set
React's act-environment flag around the rendered root to avoid false-positive
effect timing.

## Contributed by

- vk/8c71-don-t-concatenat
