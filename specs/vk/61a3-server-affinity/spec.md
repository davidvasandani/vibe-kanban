# Feature Specification: Server Affinity Sidebar Polish

**Feature dir**: `specs/vk/61a3-server-affinity/`
**Status**: Draft

## Summary

Improve the workspace Server Affinity panel so its expanded controls use the
compact spacing expected in the right sidebar and its collapsed header still
shows the current server. Operators should be able to scan placement without
opening the panel and change placement without fighting a sparse or overflowing
layout.

## User Stories

- As an operator scanning a workspace, I want the collapsed Server Affinity
  header to show the server name so that I can identify placement at a glance.
- As an operator changing affinity, I want labels and controls grouped compactly
  so that the relationship between each label and value is immediately clear.
- As an operator using a narrow sidebar, I want long server names and placement
  controls to fit safely so that the disclosure control remains usable.

## Functional Requirements

- FR-1: The Server Affinity section header MUST display concise current affinity
  context while its body is collapsed.
- FR-2: For an assigned worker, the collapsed context MUST prefer the assigned
  worker's hostname.
- FR-3: When no assigned hostname is present but a requested worker hostname is
  available, the collapsed context MUST display the requested hostname.
- FR-4: When neither hostname is available, the collapsed context MUST display
  the localized placement-kind label already represented by the workspace
  summary.
- FR-5: The interface MUST NOT invent a server label while affinity summary data
  is absent or loading.
- FR-6: Long collapsed context MUST truncate without hiding, overlapping, or
  disabling the section disclosure affordance.
- FR-7: The expanded section MUST present “Current server” and “Run on” as a
  compact aligned label/value layout.
- FR-8: The placement selector MUST use the available value-column width and
  MUST NOT overflow the supported workspace sidebar width.
- FR-9: Automatic, coordinator/local, worker, unavailable worker, and running
  workspace behavior MUST retain their existing meaning and mutation flow.
- FR-10: Collapsing the section MUST NOT require a dedicated affinity request
  or keep the body mounted solely to supply header context.
- FR-11: Existing translated labels, keyboard behavior, and accessible section
  interaction MUST remain intact.

## Out of Scope

- Changing placement scheduling or eligibility.
- Changing stop, migrate, restart, or retry behavior.
- Adding affinity data to backend schemas.
- Redesigning Server Metrics or other sidebar sections.
- Changing homelab deployment configuration or any other service.

## Acceptance Criteria

- [ ] Collapsing Server Affinity for a workspace assigned to `think4` leaves
  `think4` visible in the header.
- [ ] An assigned hostname, requested hostname, and placement-kind-only summary
  each render the required label precedence.
- [ ] A long hostname truncates within the header while the disclosure control
  remains visible and clickable.
- [ ] Expanded “Current server” and “Run on” rows use a compact aligned layout
  without the oversized whitespace shown in the task screenshot.
- [ ] The selector remains contained at the supported narrow sidebar width.
- [ ] Affinity selection and running-workspace confirmation behavior are
  unchanged in focused regression checks.
- [ ] Frontend format, type-check, lint, and focused tests pass.

## Open Questions

- None.
