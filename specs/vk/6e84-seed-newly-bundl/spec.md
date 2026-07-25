# Feature Specification: Incremental Bundled Pipeline Seeding

**Feature dir**: `specs/vk/6e84-seed-newly-bundl/`
**Status**: Implemented

## Summary

Make newly shipped bundled pipeline definitions appear automatically on
existing Vibe Kanban installations while preserving user deletions and local
edits to previously bundled files. Seed reconciliation must not leave its
bookkeeping claiming success after only part of an update was applied.

## User Stories

- As an existing user, I want a pipeline newly bundled by an application update
  to appear automatically so that I can use new workflows without resetting all
  defaults.
- As a user who deleted a bundled pipeline, I want it to stay deleted so that
  application startup does not undo my customization.
- As a user who edited a bundled pipeline, I want those edits preserved when
  other bundled pipelines are introduced.
- As an operator, I want failed seed reconciliation to retry safely so that an
  interrupted update does not permanently skip a bundled pipeline.

## Functional Requirements

- FR-1: A directory with no pipeline TOMLs must receive every currently bundled
  pipeline.
- FR-2: A non-empty installation must receive bundled pipeline IDs introduced
  after its last successful seed reconciliation.
- FR-3: An absent bundled pipeline already known to the installation must remain
  absent.
- FR-4: Automatic seeding must never overwrite an existing pipeline file.
- FR-5: Seed state must advance only after every pipeline required by that
  reconciliation has been created successfully.
- FR-6: An installation created before seed-state tracking existed must be
  upgraded without resurrecting deletions of the historical bundled defaults.
- FR-7: Repeated reconciliation with an unchanged bundle set must be
  idempotent.
- FR-8: Existing explicit reset-one and reset-all behavior must remain
  available.

## Out of Scope

- Detecting whether an existing bundled pipeline was edited.
- Restoring user-deleted bundled pipelines without an explicit reset.
- Changing pipeline API contracts or picker UI.
- Synchronizing pipeline configuration between machines.

## Acceptance Criteria

- [x] An existing directory representing the pre-`parallel-subagents` bundle
      gains `parallel-subagents.toml` on the next `ensure_seeded`/load call.
- [x] Deleting a bundled pipeline after a successful reconciliation and loading
      again leaves it deleted.
- [x] Editing one bundled pipeline and reconciling leaves its bytes unchanged.
- [x] Fresh and TOML-empty directories contain all bundled pipeline files after
      reconciliation.
- [x] A failed multi-file reconciliation does not commit seed state for files
      that were not successfully created.
- [x] Focused Rust tests pass.

## Open Questions

None. The clarification decisions are recorded in `clarifications.md`.
