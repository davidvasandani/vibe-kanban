# Feature Specification: Mobile Deploy Status

**Feature dir**: `specs/vk/7596-deploy-status-mo/`
**Status**: Clarified

## Summary

Add the currently deployed Git revision and the time elapsed since that deployment to the mobile header so an operator can confirm deployment identity and freshness without leaving the current screen.

## User Stories

- As a Vibe Kanban operator using a phone, I want to see the running revision so that I can confirm whether the expected change is live.
- As a Vibe Kanban operator, I want to see how long ago the running revision was deployed so that I can quickly distinguish a fresh rollout from an older instance.
- As a developer using an unstamped local build, I want the header to represent that state honestly so that it does not link me to a nonexistent commit.

## Functional Requirements

- FR-1: The system must make the running deployment revision available to the mobile header.
- FR-2: The system must make the release build/publish timestamp associated with the running deployment available to the mobile header when known. This is the existing deployment contract's `built_at` concept; it is not process uptime.
- FR-3: The mobile header must show a compact short revision and elapsed-time label when both values are known.
- FR-4: The elapsed-time label must remain current while the page stays open.
- FR-5: A real revision must offer direct navigation to the exact source commit.
- FR-6: An unstamped development revision must be shown as development state and must not offer a misleading commit link.
- FR-7: Missing or invalid timestamp metadata must degrade gracefully while retaining any valid revision information.
- FR-8: The deployment indicator must not prevent existing mobile navigation, settings, command, notification, sync-status, or user controls from being used.
- FR-9: The deployment indicator must have an accessible description that expands compact elapsed-time wording.
- FR-10: Existing detection and prompting for a newly deployed version must continue to work.
- FR-11: At widths where all compact status text cannot fit without displacing controls, the elapsed-time portion must hide before the revision; revision identity has priority.

## Out of Scope

- Deployment history, rollback controls, or release notes.
- Changes to services other than Vibe Kanban.
- Changes to the deployment reconciler or rollout policy.
- Reworking desktop navigation or its update controls.

## Acceptance Criteria

- [ ] At a representative phone viewport, the mobile header visibly contains the running short Git revision and a compact elapsed time.
- [ ] Activating a production revision opens the matching `davidvasandani/vibe-kanban` commit.
- [ ] Advancing time updates the displayed elapsed value without reloading the page.
- [ ] An unstamped `dev` build is non-linking and does not display fabricated deployment age.
- [ ] Missing or malformed deployment-time metadata produces no broken link, invalid date, or empty interactive control.
- [ ] Existing mobile header controls remain present and usable at representative phone widths.
- [ ] Existing deployed-version refresh detection remains unchanged and passing.

## Clarifications

- “Time since deployed” uses the immutable release's build/publish time already represented as `built_at`, because the running artifact is selected by the release flip. A later service restart does not make the code newly deployed.
- On extremely narrow headers, retain the SHA and hide elapsed time first. Existing navigation and utility controls remain higher priority than either status fragment.

## Open Questions

None.
