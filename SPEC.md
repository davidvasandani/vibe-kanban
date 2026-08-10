# Deploy Status Mobile Header

## Problem

The desktop application rail exposes the deployed Vibe Kanban revision, but
the rail is not rendered on mobile. The mobile header therefore gives an
operator no quick way to identify the running revision or understand how long
that deployment has been running.

## Goal

Expose the current deployment's short Git SHA and a human-readable elapsed
deployment age in the mobile header, while preserving the existing navigation
and header actions at narrow viewport widths.

## Scope

- Vibe Kanban service source only.
- Extend the existing system-information path only if the current API does not
  expose an authoritative deployment timestamp.
- Add a compact deployment-status presentation to the mobile navbar/header.
- Keep the desktop application rail behavior intact.
- Add focused automated coverage for data propagation, formatting, and mobile
  rendering where supported by the existing test setup.

## Requirements

1. On mobile, the header shows the running deployment's short Git SHA when it
   is available.
2. On mobile, the header shows how long ago the current server deployment
   started, using a compact human-readable value that updates as time passes.
3. The status remains legible without displacing the drawer, navigation,
   settings, help, notification, or user controls shown in the supplied mobile
   layout.
4. The SHA is sourced from the same authoritative system information used by
   the desktop application rail; development/fallback values degrade safely.
5. Deployment age is based on server/deployment start time rather than browser
   page-load time or Git commit time.
6. Existing desktop behavior and update/refresh indicators remain unchanged.
7. Generated TypeScript types are regenerated from their Rust definitions if
   the system-information contract changes.

## Acceptance Criteria

- At a mobile breakpoint, a production build with deployment metadata renders
  both a short SHA and an elapsed age in the header.
- Reloading the browser does not reset the displayed deployment age.
- A development build without an embedded Git SHA renders a safe fallback and
  does not throw.
- The header remains usable at the narrow width represented by the supplied
  iPhone screenshot.
- Relevant frontend and backend type checks/tests pass.

## Non-goals

- Changing the homelab deployment module or any other service.
- Adding deployment history, rollback controls, or a deployment dashboard.
- Redesigning the desktop application rail or the rest of the mobile navbar.

## Risks and Constraints

- Mobile horizontal space is limited; the status must be compact and may need
  a tooltip/title for expanded detail.
- The elapsed label needs a bounded refresh cadence so it stays useful without
  causing unnecessary rendering work.
- Any API addition must remain backward-compatible for callers while generated
  shared types remain the source of truth.
