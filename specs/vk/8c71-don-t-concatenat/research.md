# Research: Single-Value Browser Titles

## Existing behavior

`usePageTitle` filters truthy title parts, joins every surviving part with
` - `, and appends ` | Vibe Kanban`. `ProjectKanban` is the only current caller
that provides multiple parts (`issue?.title, projectName`). Workspace routes
provide a single workspace or create-mode label. The remote review page assigns
its own specialized pull-request title and is outside the shared hook.

## Decision: ordered fallback selection

Retain the variadic hook signature but give it fallback semantics. This avoids
duplicating issue-loading fallback logic at the page call site and makes the
non-concatenation rule enforceable at the one shared browser-title boundary.

A candidate is meaningful when `candidate.trim().length > 0`; the selected
browser title is that trimmed value. This prevents visually blank or padded
metadata without rewriting the persisted user-authored label.

## Alternatives considered

- **Accept exactly one argument**: forces every caller to resolve loading
  fallbacks before calling the hook and spreads one convention across routes.
- **Keep the product-name suffix**: still concatenates repeated context and
  directly conflicts with the requested screenshot outcome.
- **Build `ticket ID + issue title`**: makes the most constrained browser-tab
  surface noisier; the ID already remains available in visible breadcrumbs.
- **Change workspace names or breadcrumbs**: targets persisted/visible UI rather
  than the browser metadata shown in the task and conflicts with the breadcrumb
  identity contract.

## Dependencies

No new dependencies. React, React DOM, Vitest, and jsdom support already exist
in the workspace dependency graph and testing conventions.
