<!--
SpecKit project constitution (vibe-kanban).
The Specify / Plan / Analyze stages read this file and check work against it.
-->

# Project Constitution — vibe-kanban

## Core Principles

### I. Clarity over cleverness
Code and specs are written to be read. Prefer the obvious solution; match the
comment density, naming, and idiom of the surrounding code. Justify any
non-obvious choice in the spec or plan.

### II. Test the contract
Every feature defines how we will know it works (acceptance criteria) before it
is implemented. Rust logic gets `#[cfg(test)]` unit tests; UI/section changes
get a rendered-DOM component test where one already exists for that surface. No
feature is "done" without a checkable validation.

### III. Small, reversible steps
Ship the smallest change that delivers value. Prefer reusing an existing
component (e.g. `JiraBadge`) and an existing data source (the
`PROJECT_JIRA_LINKS_SHAPE` shape / `getJiraLinkForIssue`) over adding new
plumbing. Avoid speculative generality.

### IV. Shared-component boundaries are law
`packages/ui` presentational components (e.g. `KanbanIssuePanel.tsx`) own their
own layout and section order; containers in `web-core` only supply data via
render props. A change to a shared `packages/ui` component affects both
local-web and remote-web — treat both frontends as the blast radius.

### V. Don't rebuild what shipped
The bidirectional Jira reconciler (`crates/remote/src/jira/`) already exists and
is covered by the knowledge base (`wiki/external-connector-sync.md`). Features
that touch Jira sync must extend that machinery, not fork it.

## Constraints
- Follow the existing architecture and conventions of the repository.
- Do not introduce new top-level dependencies without recording the reason in
  the plan's research notes.
- Generated files (`shared/types.ts`, `shared/remote-types.ts`) are never edited
  by hand; regenerate via the `generate-types` scripts.
- Run `pnpm run format` before completing a task.

## Governance
This constitution supersedes ad-hoc preferences. When a spec or plan conflicts
with it, the constitution wins or the conflict is recorded as an open question.

**Version**: 0.2.0 (scoped for the VK–Jira source-URL surfacing task)
