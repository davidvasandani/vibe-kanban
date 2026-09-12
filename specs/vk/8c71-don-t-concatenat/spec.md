# Feature Specification: Single-Value Browser Titles

**Feature dir**: `specs/vk/8c71-don-t-concatenat/`
**Task id**: `vk/8c71-don-t-concatenat`
**Status**: Clarified

## Summary

Browser tabs should show one useful page label instead of a chain of repeated
context. A specific issue or workspace title must stand on its own, keeping the
limited tab width readable and preventing ticket numbers, project labels, and
product branding from obscuring the distinguishing text.

## User Stories

- As a user with several Vibe Kanban tabs open, I want each tab to lead with and
  contain only its distinguishing page title so I can identify it quickly.
- As a user viewing an issue, I want its descriptive title to stand alone rather
  than being concatenated with its ticket number, project, or product name.
- As a user on a page whose specific record has not loaded, I want a stable
  sensible fallback instead of a blank browser title.
- As a user navigating within Vibe Kanban, I want visible breadcrumbs to retain
  their useful hierarchy even though the browser tab is concise.

## Functional Requirements

- FR-1: When a page provides a non-empty specific label, the browser title MUST
  equal that label and no other text.
- FR-2: Browser titles MUST NOT concatenate a ticket identifier, project name,
  product name, separator, or other context onto a page-specific label.
- FR-3: Pages MAY provide an ordered set of fallback labels, but the browser
  title MUST select only the first label containing at least one non-whitespace
  character and MUST remove that label's surrounding whitespace.
- FR-4: If every page-specific fallback is absent or blank, the browser title
  MUST be `Vibe Kanban`.
- FR-5: Navigating between records or receiving a newly loaded label MUST update
  the browser title to the newly selected single value.
- FR-6: Visible breadcrumbs and issue identity labels MUST retain their existing
  hierarchy and ticket identifiers.
- FR-7: An open issue MUST prefer its descriptive issue title; when no issue
  title is available, its project page MUST fall back to the project name.

## Out of Scope

- Changing visible breadcrumb content or navigation.
- Removing issue/ticket identifiers from cards, issue panels, or URLs.
- Renaming projects, issues, or workspaces.
- Changing the dedicated pull-request review page title.
- Modifying deployment configuration or another service.

## Acceptance Criteria

- [ ] An issue titled `Fix stale execution status` sets the browser title to
      exactly `Fix stale execution status`.
- [ ] A page-specific browser title has no ` | Vibe Kanban` suffix.
- [ ] No issue ticket/simple ID is added to a descriptive issue title.
- [ ] A project page without a loaded/open issue title uses only its project
      name.
- [ ] A workspace page uses only its workspace name.
- [ ] Missing and blank page labels yield exactly `Vibe Kanban`.
- [ ] Changing the selected page label updates the existing tab title without
      leaving any prior title fragment behind.
- [ ] Visible workspace breadcrumbs continue to include the linked issue's
      ticket ID where currently required.

## Open Questions

None. See `clarifications.md`.
