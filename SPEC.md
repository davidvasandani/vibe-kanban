# Single-Value Browser Titles — Technical Specification

## Summary

Stop composing browser-tab titles from multiple labels. When a page has a
specific title, the tab must show that value alone; fallback labels such as the
project name, application name, and especially an issue/ticket identifier must
not be concatenated onto it.

## Problem

The shared `usePageTitle` hook currently accepts multiple title parts, joins
them with ` - `, and then appends ` | Vibe Kanban`. A project issue page can
therefore combine the issue title, project name, and product name in one browser
tab. These repeated context labels consume scarce tab width and obscure the
one label users need to distinguish the page. Ticket numbers are particularly
noisy when combined with an already descriptive issue title.

## Scope

- Change browser-tab title selection in the Vibe Kanban web frontend.
- Select one non-empty page title rather than joining title parts.
- Show `Vibe Kanban` only when no page-specific title is available.
- Remove redundant multi-part title arguments from current call sites.
- Add focused regression coverage for title selection and updates.

## Out of Scope

- Visible navbar breadcrumbs, workspace names, issue-card labels, branch names,
  or URL structure.
- Renaming issues, projects, workspaces, or the Vibe Kanban product.
- Remote review-page behavior, which owns a separate title implementation.
- Homelab deployment changes or changes to any other service.

## Functional Requirements

1. A non-empty page-specific label MUST become the complete `document.title`.
2. The hook MUST NOT append the application name, project name, ticket number,
   separators, or any other contextual label to a page-specific title.
3. When no non-empty page-specific label exists, `document.title` MUST be
   `Vibe Kanban`.
4. Existing workspace, create-workspace, and issue navigation MUST continue to
   update the title as their selected records change.
5. The project issue page MUST use the issue title alone when an issue is open
   and the project name alone when no issue title is available.
6. Blank strings MUST be treated as absent values, and surrounding whitespace
   MUST be removed from the selected label.

## Acceptance Criteria

- Opening an issue titled `Fix stale execution status` produces exactly
  `Fix stale execution status` as the browser title.
- The browser title contains no ` | Vibe Kanban` suffix.
- An issue ticket/simple ID is not added to its descriptive title.
- A project board with no open issue uses its project name as the single title.
- A workspace uses its workspace name as the single title.
- A loading or otherwise untitled page falls back to exactly `Vibe Kanban`.
- Focused automated tests cover specific-title, fallback-title, blank-title,
  and rerender behavior.

## Implementation Notes

Keep the change centered on
`packages/web-core/src/shared/hooks/usePageTitle.ts`. The hook can retain
ordered fallback arguments for callers that need them, but it must choose the
first non-empty value rather than concatenate the values. Update the project
page call so its argument order expresses `issue title -> project name` as a
fallback chain. Do not alter visible breadcrumbs; the screenshot's breadcrumb
context remains useful in the page chrome even though it is redundant in the
browser tab.
