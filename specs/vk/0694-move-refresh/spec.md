# Feature Specification: Move deployment refresh

**Feature dir**: `specs/vk/0694-move-refresh/`
**Status**: Clarified

## Summary

Move desktop web-deployment identity and refresh behavior away from the account
area beneath the user's headshot and into the workspace Deploy Status accordion,
so operational deployment state has one clear home without disturbing native
application updates or mobile deployment identity.

## User Stories

- As a workspace user, I want the available deployment refresh action beside
  deployment status so that I know what the action relates to.
- As a workspace user, I want the account rail to end at my profile identity so
  that revision hashes and operational actions are not mixed with account UI.
- As a desktop application user, I want installable native updates to remain
  available so that this web-refresh relocation does not remove application
  update capability.

## Functional Requirements

- **FR-1**: The desktop account/AppBar area must not show a deployed git
  revision below the user headshot.
- **FR-2**: The desktop account/AppBar area must not show the web-deployment
  Refresh action below the user headshot.
- **FR-3**: The existing native application Update action must remain in its
  current desktop AppBar location and retain its behavior.
- **FR-4**: Deploy Status must be a collapsible section at the top of the
  workspace right sidebar.
- **FR-5**: Deploy Status must show the available deployed revision and elapsed
  deployment age in its header.
- **FR-6**: When the application detects a newer web deployment, Deploy Status
  must show a Refresh action.
- **FR-7**: Activating Refresh must reload the current page through the existing
  refresh behavior and must not toggle the section as a side effect.
- **FR-8**: When no newer web deployment is detected, Deploy Status must not
  show the Refresh action.
- **FR-9**: The mobile deployment-status experience must remain available and
  unchanged.
- **FR-10**: Existing workspace right-sidebar sections must retain their order,
  visibility, sizing, and collapse behavior after Deploy Status.

## Out of Scope

- Changing how newer deployments are detected.
- Changing revision or deployment-age sources and formatting.
- Moving or redesigning native application updates.
- Changing deployment infrastructure or non-Vibe-Kanban services.

## Acceptance Criteria

- [ ] With no native update and no newer web deployment, nothing is rendered
      below the desktop user headshot for deployment identity or refresh.
- [ ] With a newer web deployment available, the desktop AppBar still renders
      no Refresh action while Deploy Status renders one.
- [ ] Clicking Deploy Status Refresh invokes the supplied page-refresh callback
      once and leaves the accordion expansion state unchanged.
- [ ] With no newer web deployment, Deploy Status has no Refresh action.
- [ ] Deploy Status is rendered as the first right-sidebar accordion and its
      header includes the revision and compact elapsed age when supplied.
- [ ] A native application update still renders the AppBar Update action and
      invokes its existing update callback.
- [ ] Existing right-sidebar rendered-DOM behavior and mobile deployment status
      tests continue to pass.

## Open Questions

- None identified. The supplied screenshots and existing product behavior make
  the ownership and conditional behavior explicit.

## Clarifications

- This task intentionally supersedes the earlier desktop Deploy Status choice
  to use a permanent, non-collapsible row. “Accordion” means Deploy Status now
  uses the right sidebar's standard disclosure section.
- Deployment identity remains in the Deploy Status header so it is visible when
  the accordion body is collapsed.
- “Refresh function” means the existing newer-web-deployment page reload, not
  the native desktop binary updater and not a newly introduced data refresh.
- A section-header action owns Refresh so activation is independent of the
  disclosure control and does not toggle expansion.
