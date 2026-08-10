# Feature Specification: Mobile Deploy Status

**Feature dir**: `specs/vk/6e4c-deploy-status-mo/`
**Status**: Implemented

## Summary

Add compact deploy status to Vibe Kanban's mobile application header so an
operator can identify the running Git revision and see how long the current
deployment has been active without switching to a desktop layout.

## User Stories

- As a mobile Vibe Kanban operator, I want to see the deployed Git SHA in the
  header so that I can confirm which revision I am using.
- As a mobile Vibe Kanban operator, I want to see the elapsed deployment age so
  that I can quickly tell whether a recent release is live.
- As a mobile user, I want deployment status to remain compact so that the
  existing navigation and account actions remain usable.

## Functional Requirements

- FR-1: The mobile application header must display the current deployment's
  short Git SHA when deployment identity is available.
- FR-2: The mobile application header must display a compact, human-readable
  elapsed age for the running deployment.
- FR-3: Deployment age must be measured from the running server deployment's
  start, and a browser reload must not reset it.
- FR-4: The elapsed age must update while the page remains open at a cadence
  appropriate to the precision shown.
- FR-5: The mobile presentation must consume the same deployment revision value
  used by the desktop deployment indicator.
- FR-6: Missing or development-only deployment metadata must degrade safely
  without preventing the mobile header or application from rendering.
- FR-7: Existing mobile header controls must remain visible, operable, and
  accessible at the narrow phone width represented by the supplied screenshot.
- FR-8: Existing desktop deployment identity and update/refresh behavior must
  remain unchanged.

## Out of Scope

- Deployment history, rollback, or deployment management controls.
- Changes to the homelab deployment module or another service.
- A redesign of the desktop application rail or mobile navigation.

## Acceptance Criteria

- [ ] At a mobile breakpoint with production deployment metadata, the header
      renders both the short SHA and an elapsed deployment age.
- [ ] Reloading the page retains an age derived from the server deployment,
      rather than restarting the age at zero.
- [ ] Advancing time updates the displayed age without another server request or
      full page reload.
- [ ] Missing timestamp metadata and the `dev` SHA sentinel do not throw or
      prevent mobile navigation from rendering.
- [ ] The narrow mobile header retains its drawer, navigation/help/settings, and
      user controls with no horizontal page overflow.
- [ ] Desktop deployment revision, update, and refresh indicators behave as
      before.
- [ ] Automated contract/formatting/render tests and repository checks relevant
      to the changed surfaces pass.

## Open Questions

None. See `clarifications.md` for the resolved presentation decisions.
