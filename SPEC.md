# Technical Specification: Search by Issue ID

## Problem

Global Search currently searches organizations, projects, cloud workspaces, local
workspaces, and persisted chat text. It does not return issue records. A user who
enters a visible issue identifier such as `VAS-602` or `SWE-163` therefore sees
no result even though the issue is accessible and can be opened directly from a
project route.

## Scope

Extend the Vibe Kanban Global Search feature so an authenticated user can find an
accessible remote issue by its human-readable `simple_id`. Issue results must be
returned by the existing remote global-search endpoint, rendered in their own
group in the existing dialog, and navigate to the issue detail route.

This change is confined to the `vibe-kanban` repository. It does not require a
deployment or hosting change in `homelab/modules/vibe-kanban-rebuild.nix`.

## Functional requirements

1. The remote `GET /v1/global-search?q=...` search includes issues belonging to
   projects in organizations accessible to the requesting user.
2. Matching is case-insensitive and literal, consistent with existing metadata
   search. A full visible identifier such as `VAS-602` must match its issue.
3. Issue results expose the issue UUID as both result identity and `issue_id`,
   the owning project UUID as `project_id`, the issue title, its `simple_id`, and
   enough organization/project context to disambiguate results.
4. At most 20 issue matches are returned, and a 21st match sets the existing
   `truncated` flag.
5. The Global Search dialog displays issue results under an `Issues` heading and
   makes the visible identifier apparent alongside the title/context.
6. Selecting an issue result changes organization through the existing shell
   coordination hook, closes the dialog, and navigates to
   `/projects/{project_id}/issues/{issue_id}`.
7. Existing organization, project, workspace, chat, partial-failure, deadline,
   deduplication, and navigation behavior remains unchanged.

## Authorization and data boundaries

- Issue search must use the existing remote membership scope; issues outside the
  requester's accessible organizations must never be returned.
- Search remains remote-metadata-only for issues. Local host fan-out must not be
  expanded or made responsible for issue records.
- The existing PostgreSQL transaction-local three-second statement timeout and
  request length limits remain in force.

## Technical approach

- Add an issue branch to the remote global-search SQL, joining `issues` to
  accessible `projects` and organizations and matching `issues.simple_id`.
- Populate the existing result envelope without introducing a new endpoint.
- Extend the TypeScript `GlobalSearchResult.kind` union and dialog category map
  with `issue`. Reuse `searchResultHref`, whose project/issue route behavior is
  already suitable for issue results.
- Update focused Rust/SQL integration and frontend unit tests for matching,
  authorization/category limits, display, and navigation URL generation.
- Regenerate SQLx offline metadata only if the repository's query-checking setup
  requires it for this dynamically constructed query.

## Acceptance criteria

- Searching for an accessible issue's exact `simple_id`, including lowercase
  input, produces an `Issues` result containing the identifier and title.
- Clicking that result opens the correct project's issue detail page.
- An issue in an organization the requester cannot access does not appear.
- Searches that do not match an issue retain current results and behavior.
- Relevant frontend tests, remote backend tests, formatting, and repository
  checks pass.

## Out of scope

- Searching issue title, description, comments, tags, external Jira identifiers,
  or internal UUIDs.
- Changing the kanban-board filter.
- Changing deployment configuration or any service other than Vibe Kanban.
