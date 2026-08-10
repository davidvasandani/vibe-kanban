# Technical Spec: Desktop Deploy Status (`VAS-377`)

## Objective

Show the running Vibe Kanban deployment identity and age at the top of the
desktop workspace right drawer. The status is informational, has no visibility
toggle, and remains present whenever the desktop right drawer is mounted.

## Existing Capability

VAS-377's mobile work already supplies the required data and presentation:

- `GET /api/info` exposes the running `version` and optional
  `deployment_timestamp`.
- `useUserSystem` owns the loaded system metadata.
- `packages/ui/src/components/DeployStatus.tsx` renders the revision, optional
  elapsed age, production commit link, development-build fallback, accessible
  label, and minute-scale refresh.

The desktop change must reuse those contracts. It must not add another request,
timestamp source, persisted preference, or deployment/IaC change.

## User Experience

On desktop workspace pages, the deploy status appears before every existing
collapsible section in the right drawer. It is a fixed, non-collapsible row and
therefore cannot be hidden independently of the drawer itself.

The row:

- is labelled `Deploy Status`;
- shows the same revision and elapsed-age presentation used by the mobile
  header;
- links a real revision to its exact GitHub commit;
- renders `dev` as a non-linking development build;
- retains a valid revision when the timestamp is missing or malformed;
- renders no misleading placeholder when no version is available; and
- visually participates in the drawer's existing divided stack while remaining
  above scrollable/collapsible feature sections.

“Always visible” means the deploy-status row has no feature toggle and no
collapse state. The existing global right-drawer toggle still controls whether
the drawer itself is open.

## Architecture

The workspace `RightSidebar` is the desktop right-drawer composition boundary.
It will read `appVersion` and `deploymentTimestamp` from the existing
`useUserSystem` context and render a small desktop-specific row using the shared
`DeployStatus` component before mapping its current section definitions.

The shared component may receive additive styling/presentation options if the
desktop row needs a wider layout than the compact mobile header, but mobile
behavior and its responsive priority must remain unchanged.

The project/issue contextual side panel is out of scope: it is route content,
not the persistent workspace drawer controlled by `ToggleRightSidebar`.

## Testing

Automated coverage will verify that:

1. The desktop drawer renders `Deploy Status` before its existing sections.
2. The row is non-collapsible and does not introduce a toggle or persisted
   preference.
3. Existing deployment metadata is passed to the shared presentation.
4. Production, `dev`, and missing/invalid timestamp behavior remains correct.
5. Existing mobile navbar behavior is unchanged.

Verification will use the repository-owned frontend test, type-check, lint,
format, and generated-type checks appropriate to the touched files.

## Out of Scope

- Changes to any service other than Vibe Kanban.
- Changes to `homelab/modules/vibe-kanban-rebuild.nix` or any host deployment.
- Deployment history, rollback controls, release notes, or a status toggle.
- New backend fields, API calls, dependencies, or persisted UI preferences.
- Redesigning the existing right drawer or project/issue contextual panels.

## Acceptance Criteria

- [ ] On desktop, the workspace right drawer starts with a visible `Deploy
      Status` row above all existing sections.
- [ ] The row displays the running revision and deployment age using the VAS-377
      data already loaded by the application.
- [ ] The status has no collapse control, feature toggle, or independent hidden
      state.
- [ ] A production revision links to the exact Vibe Kanban commit; `dev` does
      not link.
- [ ] Missing or invalid deployment time degrades without `Invalid Date` or a
      fabricated age.
- [ ] The existing mobile deploy status and desktop drawer controls continue to
      behave as before.
