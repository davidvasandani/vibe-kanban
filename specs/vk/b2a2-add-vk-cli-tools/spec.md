# Feature Specification: CLI Tools in Workspace Sessions

**Feature dir**: `specs/vk/b2a2-add-vk-cli-tools/`
**Status**: Draft

## Summary

Make CLI tools installed through Vibe Kanban available by command name in newly
started workspace sessions. Users should receive the tools the settings screen
reports as installed without losing machine-provided commands or existing
environment configuration, regardless of which Vibe Kanban execution host runs
the workspace.

## User Stories

- As a workspace user, I want a CLI installed through Vibe Kanban to be
  available in my new workspace session so that I can use it without locating
  or invoking its installation path manually.
- As an operator, I want machine-provided CLI copies to keep precedence so that
  app-managed tools do not unexpectedly replace host policy or packaging.
- As a clustered-deployment user, I want workspace sessions to behave the same
  on their assigned execution host so that tool availability does not depend on
  whether work runs locally or remotely.

## Functional Requirements

- FR-1: A newly started workspace session MUST be able to resolve a CLI tool
  that Vibe Kanban has installed and exposed for the session's execution host.
- FR-2: The system MUST preserve all existing executable-search locations when
  adding Vibe Kanban-managed tools.
- FR-3: Machine-provided executable-search locations MUST take precedence over
  the Vibe Kanban-managed tools location.
- FR-4: The managed tools location MUST appear no more than once in a workspace
  session's executable-search path.
- FR-5: If the managed tools location is unavailable on the execution host, the
  workspace session MUST still start and its executable-search path MUST remain
  usable.
- FR-6: The behavior MUST apply to every Vibe Kanban process boundary presented
  to users as part of a workspace session, including managed agent execution and
  interactive workspace terminals.
- FR-7: In a clustered deployment, the managed tools location MUST be valid for
  the worker that actually starts the workspace process; a coordinator-only
  location MUST NOT be advertised as usable on another node.
- FR-8: Only the designated managed executable directory MUST be exposed; tool
  install staging areas, credentials, and unrelated application data MUST NOT
  be added to the executable-search path.
- FR-9: The feature MUST retain the existing rule that workspace- or
  organization-provided environment values cannot override reserved runtime
  environment names.
- FR-10: The change MUST remain confined to the Vibe Kanban service and its
  governing deployment configuration.

## Out of Scope

- Adding, removing, or upgrading catalog tools.
- Changing tool installation, authentication, credential storage, or settings
  UI behavior.
- Updating the environment of workspace processes that are already running.
- Changing any service other than Vibe Kanban.

## Acceptance Criteria

- [ ] Starting a new workspace session after installing a managed CLI allows a
      command lookup for that CLI to resolve successfully.
- [ ] When machine and managed copies share a command name, command lookup
      resolves the machine copy first.
- [ ] A session with a custom executable-search path retains every custom entry
      and contains only one managed tools entry.
- [ ] A session starts successfully with an unchanged usable path when the
      managed tools directory is absent.
- [ ] Managed agent processes and interactive workspace terminals satisfy the
      same tool-availability contract.
- [ ] A clustered session uses a path that is valid on its assigned worker and
      does not rely on a coordinator-only asset path.
- [ ] Automated tests cover ordering, preservation, deduplication, missing-path
      behavior, and the affected local and clustered process boundaries.

## Open Questions

None. See `clarifications.md` for the resolved scope and clustered-host policy.
