# Feature Specification: Claude Fable 5.1 and Current Claude Code

**Feature dir:** `specs/vk/1f95-update-claude-mo/`  
**Status:** Implemented and reviewed

## Summary

Vibe Kanban users need the newly released Claude Fable 5.1 model to appear in
the Claude model picker and need Claude executions to run through the current,
explicitly reviewed Claude Code release. The refresh must retain existing model
choices and Vibe Kanban's safety behavior.

## User Stories

- As a Vibe Kanban user, I want to select Fable 5.1 for a Claude execution so
  that I can use the new model without entering a custom command or model ID.
- As a Vibe Kanban operator, I want Claude executions pinned to the latest
  reviewed Claude Code release so that users receive current model support and
  fixes without an unbounded runtime dependency.
- As a Vibe Kanban maintainer, I want dependency-sensitive safety assumptions
  checked during the upgrade so that a routine CLI refresh does not silently
  reopen unsupported background behavior.

## Functional Requirements

- **FR-1:** The Claude model selector MUST list Fable 5.1 under the exact model
  identifier supported by the current Claude Code release.
- **FR-2:** Selecting Fable 5.1 MUST pass that identifier unchanged to Claude
  Code for initial and resumed executions.
- **FR-3:** Fable 5.1 MUST offer all Claude reasoning-effort choices that its
  executor classification supports.
- **FR-4:** Fable 5.1 context utilization MUST use its native 1M-token context
  window before the end-of-turn usage report is available.
- **FR-5:** Existing supported Claude model selections and the default model
  MUST remain available.
- **FR-6:** Vibe Kanban MUST invoke a fixed, current Claude Code release whose
  release notes and alias behavior were reviewed for this task.
- **FR-7:** Version-specific documentation, checks, and safety assumptions MUST
  identify the same Claude Code release.
- **FR-8:** Unsupported wake-up and in-turn background execution behavior MUST
  remain blocked, including the parameter-level background command path.
- **FR-9:** The Vibe Kanban deployment MUST continue to supply everything the
  refreshed Claude executor requires without changing another service.
- **FR-10:** Automated checks MUST detect removal of Fable 5.1, loss of its
  reasoning choices, incorrect context-window classification, version-pin
  drift, or drift in the reviewed native safety identifiers.

## Out of Scope

- Updating model catalogs for non-Claude-Code executors unless they already
  consume the exact same catalog.
- Changing which Claude model is Vibe Kanban's default.
- Enabling vendor background jobs, scheduled wake-ups, or other lifecycle
  behavior that cannot survive a Vibe Kanban turn.
- Updating or deploying unrelated homelab services.

## Acceptance Criteria

- [x] The Claude picker contains a Fable 5.1 entry with a vendor-verified ID.
- [x] The Fable 5.1 entry exposes the expected effort choices.
- [x] Fable 5.1 is classified with its native 1M-token context window.
- [x] Initial and follow-up command construction accepts the selected ID
      unchanged through the existing model argument path.
- [x] The standard executor command contains one fixed latest-reviewed Claude
      Code version, with every version twin updated consistently.
- [x] Existing model choices and the default remain unchanged.
- [x] Native tool names/aliases used by Vibe Kanban controls are verified
      against the pinned executing artifact and guarded by tests.
- [x] Focused tests, generated-type checks, formatting, and relevant broader
      repository checks pass.
- [x] The governing Nix module is either verified compatible without changes
      or updated and validated only as required for Vibe Kanban.
- [x] Independent Codex review reports no significant findings.

## Open Questions

None. See `clarifications.md` for the resolved decisions and evidence.
