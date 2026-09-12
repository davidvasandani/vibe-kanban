# Feature Specification: Desktop Deploy Status

**Feature dir**: `specs/vk/992c-deploy-status/`
**Status**: Clarified

## Summary

Add deploy status to the top of the desktop workspace right drawer so an
operator can confirm which Vibe Kanban revision is running and how recently it
was deployed without leaving the current workspace. The status is permanent
drawer content rather than another configurable or collapsible section.

## User Stories

- As a desktop operator, I want the right drawer to show the running revision so
  that I can confirm the expected change is live while I work.
- As a desktop operator, I want to see the deployment age so that I can quickly
  distinguish a fresh rollout from an older instance.
- As a developer using an unstamped build, I want the status to identify the
  development state honestly without linking to a nonexistent commit.

## Functional Requirements

- FR-1: On desktop workspace pages, the system must show deploy status at the
  top of the workspace right drawer, before all existing drawer sections.
- FR-2: The status must display the running deployment revision when known.
- FR-3: The status must display the elapsed time since the running immutable
  release was built/published when that timestamp is known and valid.
- FR-4: The elapsed label must remain current while the page stays open.
- FR-5: A real revision must offer navigation to the exact source commit.
- FR-6: An unstamped `dev` revision must be non-linking and must not show a
  fabricated deployment age.
- FR-7: Missing or invalid deployment-time metadata must retain valid revision
  identity without displaying an invalid or fabricated age.
- FR-8: The status must have no collapse control, visibility toggle, or
  independent persisted hidden state.
- FR-9: The existing global right-drawer control may still open or close the
  entire drawer; whenever the drawer is mounted, its deploy-status row must be
  present.
- FR-10: Existing drawer sections must retain their expansion, scrolling, and
  available-height behavior.
- FR-11: Existing mobile deploy-status behavior must remain unchanged.
- FR-12: The status must provide an accessible description of the compact
  revision and age presentation.

## Out of Scope

- Deployment history, rollback controls, release notes, or rollout policy.
- Changes to services other than Vibe Kanban.
- Changes to homelab deployment configuration.
- New deployment metadata sources or server contracts.
- The route-specific project/issue contextual side panel.
- Redesigning the right drawer or its existing open/close control.

## Acceptance Criteria

- [ ] At a representative desktop viewport, the workspace right drawer begins
      with a visible `Deploy Status` row above every existing section.
- [ ] With production metadata, the row shows the short revision and compact
      deployment age, and activating the revision opens its exact Vibe Kanban
      commit.
- [ ] Advancing time updates the displayed deployment age without a page reload.
- [ ] A `dev` build renders a non-linking development status with no fabricated
      age.
- [ ] A missing or malformed deployment timestamp leaves the revision visible
      and displays neither `Invalid Date` nor an empty interactive control.
- [ ] The deploy-status row exposes no disclosure button, toggle, or stored
      visibility preference.
- [ ] Existing drawer sections remain reachable and preserve their current
      expanded/collapsed sizing behavior.
- [ ] Existing mobile deploy-status tests remain passing.

## Open Questions

None.

## Clarifications

- “Right drawer” means the persistent desktop workspace drawer controlled by
  the existing right-sidebar action. Route-specific project/issue contextual
  panels are separate content surfaces and will not repeat this status.
- “Always visible” means the row has no independent collapse or visibility
  state. Closing the entire right drawer through its existing global control is
  unchanged.
